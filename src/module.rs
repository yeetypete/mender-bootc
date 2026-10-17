//! Bootc Mender Update Module.
//!
//! 1. `ArtifactInstall` stages the Artifact's image with `bootc switch`.
//! 2. Mender reboots the host, and bootc finalizes the staged deployment on
//!    shutdown.
//! 3. `ArtifactVerifyReboot` and `ArtifactCommit` check that the image booted.
//!
//! On failure, `ArtifactRollback` deletes the staged deployment before the
//! reboot, and queues bootc's rollback deployment after it.
//!
//! The module's state is fully handled by bootc and read from `bootc status`.

use std::io::Write;

use anyhow::{Result, ensure};
use camino::Utf8Path;

use crate::bootc::{BootEntry, Bootc, Host};
use crate::meta_data::{MetaData, TargetImage};
use crate::protocol::{self, RebootAction, State};

/// Run `state` for the update in `file_tree`, writing query responses to
/// `out`.
///
/// # Errors
///
/// Returns the state's error, which fails the update.
pub fn run(
    state: &State,
    file_tree: &Utf8Path,
    bootc: &impl Bootc,
    out: &mut impl Write,
) -> Result<()> {
    let image = || MetaData::read(file_tree).map(|meta_data| meta_data.image);
    match state {
        State::ArtifactInstall => install(&image()?, bootc),
        // A manual call to `mender-update commit` runs `ArtifactCommit`
        // without `ArtifactVerifyReboot`, so `ArtifactCommit` also checks that
        // the reboot booted the update and fails if it did not.
        State::ArtifactVerifyReboot | State::ArtifactCommit => verify_reboot(&image()?, bootc),
        State::ArtifactRollback => rollback(&image()?, bootc),
        State::ArtifactVerifyRollbackReboot => verify_rollback_reboot(&image()?, bootc),
        State::NeedsArtifactReboot => protocol::respond(out, needs_reboot(&image()?, bootc)?),
        State::SupportsRollback => protocol::respond(out, "Yes"),
        State::Download
        | State::DownloadWithFileSizes
        | State::ArtifactReboot
        | State::Cleanup
        | State::ArtifactRollbackReboot
        | State::ArtifactFailure
        | State::ProvidePayloadFileSizes
        | State::Other(_) => Ok(()),
    }
}

/// The staged deployment, if it is `image`.
fn staged<'a>(host: &'a Host, image: &TargetImage) -> Option<&'a BootEntry> {
    host.status
        .staged
        .as_ref()
        .filter(|staged| staged.is_image(image))
}

/// Whether `image` is booted.
fn is_booted(host: &Host, image: &TargetImage) -> Result<bool> {
    Ok(host.status.require_booted()?.is_image(image))
}

/// Stage `image` for the next boot.
fn install(image: &TargetImage, bootc: &impl Bootc) -> Result<()> {
    ensure!(
        !is_booted(&bootc.status()?, image)?,
        "{image} is already booted"
    );
    tracing::info!("Staging {image}");
    bootc.switch(image)?;
    ensure!(
        staged(&bootc.status()?, image).is_some(),
        "bootc did not stage {image}"
    );
    Ok(())
}

/// Reboot to boot the staged image or the queued rollback.
fn needs_reboot(image: &TargetImage, bootc: &impl Bootc) -> Result<RebootAction> {
    let host = bootc.status()?;
    Ok(
        if staged(&host, image).is_some() || host.status.rollback_queued {
            RebootAction::Automatic
        } else {
            RebootAction::No
        },
    )
}

/// Check that the reboot booted `image`.
fn verify_reboot(image: &TargetImage, bootc: &impl Bootc) -> Result<()> {
    ensure!(
        is_booted(&bootc.status()?, image)?,
        "the reboot did not boot {image}"
    );
    Ok(())
}

/// Make the previous deployment, not `image`, boot next.
fn rollback(image: &TargetImage, bootc: &impl Bootc) -> Result<()> {
    let host = bootc.status()?;
    if is_booted(&host, image)? {
        if host.status.rollback_queued {
            tracing::info!("The rollback is already queued");
        } else {
            tracing::info!("Queueing the rollback");
            bootc.rollback()?;
        }
    } else if let Some(staged) = staged(&host, image) {
        // Delete the staged deployment so that shutdown does not finalize it.
        let verity = staged.verity()?;
        tracing::info!("Deleting the staged deployment {verity}");
        bootc.delete_deployment(verity)?;
    }
    Ok(())
}

/// Check that the reboot booted the previous deployment, not `image`.
fn verify_rollback_reboot(image: &TargetImage, bootc: &impl Bootc) -> Result<()> {
    ensure!(
        !is_booted(&bootc.status()?, image)?,
        "the reboot booted {image} instead of rolling back"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::bootc::{BootEntryComposefs, ImageReference, ImageStatus, Transport, Verity};

    /// A registry image whose digest repeats the hex digit `digit`.
    fn image(digit: char) -> TargetImage {
        let digest: String = std::iter::repeat_n(digit, 64).collect();
        format!("docker.io/example/os@sha256:{digest}")
            .parse()
            .expect("a valid image")
    }

    fn entry(image: &TargetImage, verity: &str) -> BootEntry {
        BootEntry {
            image: Some(ImageStatus {
                image: ImageReference {
                    image: image.to_string(),
                    transport: Transport::Registry,
                },
            }),
            composefs: Some(BootEntryComposefs {
                verity: Verity::new(verity),
            }),
        }
    }

    /// A bootc that keeps its deployments in memory.
    #[derive(Debug, Default)]
    struct FakeBootc {
        host: RefCell<Host>,
        rollback: RefCell<Option<BootEntry>>,
    }

    impl FakeBootc {
        /// A host that booted `image`.
        fn booted(image: &TargetImage) -> Self {
            let bootc = Self::default();
            bootc.host.borrow_mut().status.booted = Some(entry(image, "booted"));
            bootc
        }

        /// Boot the next deployment, as a reboot would.
        fn reboot(&self) {
            let status = &mut self.host.borrow_mut().status;
            let mut rollback = self.rollback.borrow_mut();
            if status.rollback_queued {
                std::mem::swap(&mut status.booted, &mut *rollback);
                status.rollback_queued = false;
            } else if let Some(staged) = status.staged.take() {
                *rollback = status.booted.replace(staged);
            }
        }
    }

    impl Bootc for FakeBootc {
        fn status(&self) -> Result<Host> {
            Ok(self.host.borrow().clone())
        }

        fn switch(&self, image: &TargetImage) -> Result<()> {
            self.host.borrow_mut().status.staged = Some(entry(image, "staged"));
            Ok(())
        }

        fn rollback(&self) -> Result<()> {
            ensure!(self.rollback.borrow().is_some(), "no rollback deployment");
            let status = &mut self.host.borrow_mut().status;
            status.rollback_queued = !status.rollback_queued;
            status.staged = None;
            Ok(())
        }

        fn delete_deployment(&self, _verity: &Verity) -> Result<()> {
            self.host.borrow_mut().status.staged = None;
            Ok(())
        }
    }

    fn reboot_action(bootc: &FakeBootc) -> Result<RebootAction> {
        needs_reboot(&image('b'), bootc)
    }

    #[test]
    fn installs_update() -> Result<()> {
        let bootc = FakeBootc::booted(&image('a'));
        install(&image('b'), &bootc)?;
        assert_eq!(reboot_action(&bootc)?, RebootAction::Automatic);
        assert!(verify_reboot(&image('b'), &bootc).is_err());

        bootc.reboot();
        verify_reboot(&image('b'), &bootc)?;
        Ok(())
    }

    #[test]
    fn rejects_booted_image() {
        let bootc = FakeBootc::booted(&image('b'));
        let err = install(&image('b'), &bootc).unwrap_err();
        assert!(err.to_string().contains("already booted"), "{err}");
    }

    #[test]
    fn deletes_staged_deployment_before_reboot() -> Result<()> {
        let bootc = FakeBootc::booted(&image('a'));
        install(&image('b'), &bootc)?;

        rollback(&image('b'), &bootc)?;
        assert!(bootc.status()?.status.staged.is_none());
        assert_eq!(reboot_action(&bootc)?, RebootAction::No);
        verify_rollback_reboot(&image('b'), &bootc)?;
        Ok(())
    }

    #[test]
    fn rolls_back_after_reboot() -> Result<()> {
        let bootc = FakeBootc::booted(&image('a'));
        install(&image('b'), &bootc)?;
        bootc.reboot();

        // Mender runs the state again after a spontaneous reboot, which must
        // not unqueue the rollback.
        rollback(&image('b'), &bootc)?;
        rollback(&image('b'), &bootc)?;
        assert_eq!(reboot_action(&bootc)?, RebootAction::Automatic);
        assert!(verify_rollback_reboot(&image('b'), &bootc).is_err());

        bootc.reboot();
        verify_rollback_reboot(&image('b'), &bootc)?;
        Ok(())
    }
}

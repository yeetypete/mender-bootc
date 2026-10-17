//! Update VMs booted from the base test image to the update image with the
//! Mender Client in standalone mode, see
//! <https://docs.mender.io/artifact-creation/standalone-deployment>:
//!
//! 1. `mender-update install` stages the update image through the bootc Update
//!    Module.
//! 2. `mender-update commit` finalizes the update after a reboot.
//! 3. `mender-update rollback` rolls back to the base image.
//!
//! Run with `cargo xtask test`.

use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mender_bootc::bootc::Host;
use mender_bootc::command::CommandRunExt;

/// The libvirt connection the VMs run in.
const LIBVIRT: &str = "qemu:///session";

/// The image the VMs boot, built by `cargo xtask images`.
const BASE_IMAGE: &str = "localhost/mender-bootc-test:base";

/// The longest a VM may take to reboot.
const REBOOT_TIMEOUT: Duration = Duration::from_secs(300);

/// The path of the directory holding the update Artifact in the VM.
const SHARED: &str = "/run/mender-bootc-test";

/// The path of the update Artifact in the VM.
const ARTIFACT: &str = "/run/mender-bootc-test/update.mender";

/// The name of the update Artifact.
const ARTIFACT_NAME: &str = "update";

/// The compatible type of the test image, see
/// `tests/images/fedora/system_files/usr/lib/tmpfiles.d/mender.conf`.
const COMPATIBLE_TYPE: &str = "mender-bootc-test";

/// A VM booted from the base image, removed when dropped.
#[derive(Debug)]
struct Vm {
    name: String,
}

impl Vm {
    /// Install the base image onto a disk and boot it as the VM of `test`.
    fn boot(test: &str, shared: &Path) -> Result<Self> {
        let vm = Self {
            name: format!("mender-bootc-test-{}", test.replace('_', "-")),
        };
        // A VM of an earlier run that failed to clean up.
        vm.remove();
        Command::new("bcvk")
            .args(["libvirt", "--connect", LIBVIRT, "run", "--name", &vm.name])
            .args(["--composefs-backend", "--filesystem", "ext4"])
            .arg("--bind")
            .arg(format!("{}:{SHARED}", shared.display()))
            .args(["--detach", "--ssh-wait", BASE_IMAGE])
            .run_inherited()?;
        Ok(vm)
    }

    /// Remove the VM, if it exists.
    fn remove(&self) {
        let _ = Command::new("bcvk")
            .args(["libvirt", "--connect", LIBVIRT, "rm", "--force", &self.name])
            .output();
    }

    /// A command that runs `cmd` as root in the VM.
    fn ssh(&self, cmd: &str) -> Command {
        let mut command = Command::new("bcvk");
        command
            .args(["libvirt", "--connect", LIBVIRT, "ssh", &self.name, "--"])
            .args(["sudo", "sh", "-c", cmd]);
        command
    }

    /// Run `cmd` in the VM, returning its stdout without the trailing newline.
    fn run(&self, cmd: &str) -> Result<String> {
        let stdout = self.ssh(cmd).run_get_output()?;
        Ok(String::from_utf8(stdout)?.trim_end().to_owned())
    }

    /// Reboot the VM and wait for it to come back.
    fn reboot(&self) -> Result<()> {
        let boot_id = self.boot_id()?;
        let _ = self.ssh("systemctl reboot").output();
        let start = Instant::now();
        while start.elapsed() < REBOOT_TIMEOUT {
            if self.boot_id().is_ok_and(|id| id != boot_id) {
                return Ok(());
            }
            thread::sleep(Duration::from_secs(1));
        }
        bail!("{} did not come back within {REBOOT_TIMEOUT:?}", self.name)
    }

    fn boot_id(&self) -> Result<String> {
        self.run("cat /proc/sys/kernel/random/boot_id")
    }

    /// Whether bootc has a deployment staged for the next boot.
    fn has_staged(&self) -> Result<bool> {
        let status = self.run("bootc status --format json --format-version 1")?;
        let host: Host = serde_json::from_str(&status)?;
        Ok(host.status.staged.is_some())
    }

    /// The `IMAGE_VERSION` of the booted image, `base` or `update`.
    fn image_version(&self) -> Result<String> {
        self.run(". /usr/lib/os-release && echo \"$IMAGE_VERSION\"")
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        self.remove();
    }
}

/// Generate an Artifact in `dir` that updates to the update image.
fn generate_artifact(dir: &Path) -> Result<()> {
    let image = std::env::var("MENDER_BOOTC_TEST_UPDATE")
        .context("MENDER_BOOTC_TEST_UPDATE is not set, run the tests with `cargo xtask test`")?;
    Command::new(env!("CARGO_BIN_EXE_bootc-artifact-gen"))
        .args(["--artifact-name", ARTIFACT_NAME])
        .args(["--compatible-types", COMPATIBLE_TYPE])
        .arg("--output-path")
        .arg(dir.join("update.mender"))
        .arg(&image)
        .run_inherited()
}

/// Boot the VM of `test` with an Artifact that updates to the update image.
fn boot_with_artifact(test: &str) -> Result<(tempfile::TempDir, Vm)> {
    let dir = tempfile::tempdir()?;
    generate_artifact(dir.path())?;
    let vm = Vm::boot(test, dir.path())?;
    Ok((dir, vm))
}

#[test]
fn installs_and_commits_update() -> Result<()> {
    let (_dir, vm) = boot_with_artifact("installs_and_commits_update")?;
    vm.run(&format!("mender-update install {ARTIFACT}"))?;
    assert_eq!(vm.image_version()?, "base");

    vm.reboot()?;
    assert_eq!(vm.image_version()?, "update");
    vm.run("mender-update commit")?;
    let provides = vm.run("mender-update show-provides")?;
    assert!(
        provides.contains(&format!("rootfs-image.bootc.version={ARTIFACT_NAME}")),
        "{provides}"
    );
    Ok(())
}

#[test]
fn rolls_back_before_reboot() -> Result<()> {
    let (_dir, vm) = boot_with_artifact("rolls_back_before_reboot")?;
    vm.run(&format!("mender-update install {ARTIFACT}"))?;
    assert!(vm.has_staged()?);
    vm.run("mender-update rollback")?;
    assert!(!vm.has_staged()?);

    vm.reboot()?;
    assert_eq!(vm.image_version()?, "base");
    Ok(())
}

#[test]
fn commit_before_reboot_rolls_back() -> Result<()> {
    let (_dir, vm) = boot_with_artifact("commit_before_reboot_rolls_back")?;
    vm.run(&format!("mender-update install {ARTIFACT}"))?;
    // The update was not booted, so the commit fails and Mender rolls back.
    assert!(vm.run("mender-update commit").is_err());
    assert!(!vm.has_staged()?);

    vm.reboot()?;
    assert_eq!(vm.image_version()?, "base");
    Ok(())
}

#[test]
fn rolls_back_after_reboot() -> Result<()> {
    let (_dir, vm) = boot_with_artifact("rolls_back_after_reboot")?;
    vm.run(&format!("mender-update install {ARTIFACT}"))?;
    vm.reboot()?;
    assert_eq!(vm.image_version()?, "update");

    vm.run("mender-update rollback")?;
    vm.reboot()?;
    assert_eq!(vm.image_version()?, "base");
    Ok(())
}

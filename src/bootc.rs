//! Run bootc and read its status.

use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::command::CommandRunExt;
use crate::meta_data::TargetImage;

/// bootc operations required for an update.
pub trait Bootc {
    /// The host's bootc deployments.
    ///
    /// # Errors
    ///
    /// Fails if bootc fails or prints an unexpected status.
    fn status(&self) -> Result<Host>;

    /// Pull `image` and stage it for the next boot. bootc finalizes the staged
    /// deployment on shutdown.
    ///
    /// # Errors
    ///
    /// Fails if bootc fails.
    fn switch(&self, image: &TargetImage) -> Result<()>;

    /// Queue the rollback deployment for the next boot, or unqueue it if it is
    /// queued.
    ///
    /// # Errors
    ///
    /// Fails if bootc fails.
    fn rollback(&self) -> Result<()>;

    /// Delete the deployment with the verity `verity`. Deleting the staged
    /// deployment cancels the pending update.
    ///
    /// # Errors
    ///
    /// Fails if bootc fails.
    fn delete_deployment(&self, verity: &Verity) -> Result<()>;
}

/// Runs the bootc CLI.
#[derive(Debug)]
pub struct BootcCli;

impl Bootc for BootcCli {
    fn status(&self) -> Result<Host> {
        Command::new("bootc")
            .args(["status", "--format", "json", "--format-version", "1"])
            .run_and_parse_json()
    }

    fn switch(&self, image: &TargetImage) -> Result<()> {
        Command::new("bootc")
            .args(["switch", "--quiet", &image.to_string()])
            .run_inherited()
    }

    fn rollback(&self) -> Result<()> {
        Command::new("bootc").arg("rollback").run_inherited()
    }

    fn delete_deployment(&self, verity: &Verity) -> Result<()> {
        Command::new("bootc")
            .args(["delete-deployment", verity.as_str()])
            .run_inherited()
    }
}

/// The fs-verity digest of a deployment's composefs image, which are used
/// to identify deployments in bootc.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Verity(String);

impl Verity {
    #[cfg(test)]
    pub(crate) fn new(verity: impl Into<String>) -> Self {
        Self(verity.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Verity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The parts of `bootc status` the module reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Host {
    pub status: HostStatus,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    /// The staged image for the next boot.
    pub staged: Option<BootEntry>,
    /// The booted image.
    pub booted: Option<BootEntry>,
    /// Set to true if the rollback entry is queued for the next boot.
    #[serde(default)]
    pub rollback_queued: bool,
}

impl HostStatus {
    /// The booted deployment.
    ///
    /// # Errors
    ///
    /// Fails if bootc reports none, e.g. outside a bootc system.
    pub fn require_booted(&self) -> Result<&BootEntry> {
        self.booted
            .as_ref()
            .context("bootc reports no booted deployment")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BootEntry {
    pub image: Option<ImageStatus>,
    pub composefs: Option<BootEntryComposefs>,
}

impl BootEntry {
    /// The deployment's verity.
    ///
    /// # Errors
    ///
    /// Fails for a non-composefs deployment.
    pub fn verity(&self) -> Result<&Verity> {
        self.composefs
            .as_ref()
            .map(|composefs| &composefs.verity)
            .context("not a composefs deployment")
    }

    /// Whether the deployment was pulled from `image`.
    #[must_use]
    pub fn is_image(&self, image: &TargetImage) -> bool {
        self.image.as_ref().is_some_and(|status| {
            status.image.transport == Transport::Registry && status.image.image == image.to_string()
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageStatus {
    pub image: ImageReference,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageReference {
    pub image: String,
    pub transport: Transport,
}

/// The containers-transports(5) an image was pulled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    Registry,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BootEntryComposefs {
    pub verity: Verity,
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOTED: &str = "docker.io/example/os@sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const STAGED: &str = "docker.io/example/os@sha256:2222222222222222222222222222222222222222222222222222222222222222";

    #[test]
    fn parses_composefs_status() -> Result<()> {
        let booted_verity = "a".repeat(128);
        let staged_verity = "b".repeat(128);
        // The fields of a composefs `bootc status` this module reads.
        let status = format!(
            r#"{{
              "status": {{
                "booted": {{
                  "composefs": {{"verity": "{booted_verity}"}},
                  "image": {{"image": {{"image": "{BOOTED}", "transport": "registry"}}}}
                }},
                "rollbackQueued": false,
                "staged": {{
                  "composefs": {{"verity": "{staged_verity}"}},
                  "image": {{"image": {{"image": "{STAGED}", "transport": "registry"}}}}
                }}
              }}
            }}"#
        );
        let host: Host = serde_json::from_str(&status)?;
        let booted = host.status.require_booted()?;
        assert_eq!(booted.verity()?, &Verity::new(booted_verity));
        assert!(booted.is_image(&BOOTED.parse()?));
        let staged = host.status.staged.as_ref().unwrap();
        assert_eq!(staged.verity()?, &Verity::new(staged_verity));
        assert!(staged.is_image(&STAGED.parse()?));
        assert!(!staged.is_image(&BOOTED.parse()?));
        Ok(())
    }
}

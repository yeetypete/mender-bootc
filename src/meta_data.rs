//! An Artifact's meta-data, set by `mender-artifact write module-image
//! --meta-data`.

use std::str::FromStr;

use anyhow::{Context, Result, bail};
use camino::Utf8Path;
use oci_spec::distribution::Reference;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetaData {
    /// The image reference to update to.
    pub image: TargetImage,
}

impl MetaData {
    /// Read the meta-data from header/meta-data in the update's file tree.
    ///
    /// # Errors
    ///
    /// Fails if the meta-data is missing or invalid.
    pub fn read(file_tree: &Utf8Path) -> Result<Self> {
        let path = file_tree.join("header/meta-data");
        let data = std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?;
        Self::parse(&data).with_context(|| format!("parsing {path}"))
    }

    fn parse(data: &str) -> Result<Self> {
        // Mender writes `null` for an Artifact without meta-data.
        serde_json::from_str::<Option<Self>>(data)?
            .context("the Artifact has no meta-data, build it with bootc-artifact-gen")
    }
}

/// An image reference pinned by digest, e.g. `docker.io/example/os@sha256:…`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct TargetImage(Reference);

impl FromStr for TargetImage {
    type Err = anyhow::Error;

    fn from_str(image: &str) -> Result<Self> {
        let reference: Reference = image
            .parse()
            .with_context(|| format!("{image} is not a valid image reference"))?;
        if reference.digest().is_none() {
            bail!("{image} is not pinned by a digest");
        }
        // containers-image cannot pull a reference with both.
        if reference.tag().is_some() {
            bail!("{image} has both a tag and a digest");
        }
        Ok(Self(reference))
    }
}

impl TryFrom<String> for TargetImage {
    type Error = anyhow::Error;

    fn try_from(image: String) -> Result<Self> {
        image.parse()
    }
}

impl std::fmt::Display for TargetImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGE: &str = "docker.io/example/os@sha256:2222222222222222222222222222222222222222222222222222222222222222";

    #[test]
    fn round_trips_image() -> Result<()> {
        let meta_data = MetaData::parse(&format!(r#"{{"image": "{IMAGE}"}}"#))?;
        assert_eq!(meta_data.image.to_string(), IMAGE);
        Ok(())
    }

    #[test]
    fn rejects_missing_meta_data() {
        let err = MetaData::parse("null").unwrap_err();
        assert!(err.to_string().contains("no meta-data"), "{err}");
    }

    #[test]
    fn rejects_image_without_digest() {
        let err = "docker.io/example/os:latest"
            .parse::<TargetImage>()
            .unwrap_err();
        assert!(err.to_string().contains("not pinned"), "{err}");
    }

    #[test]
    fn rejects_tag_and_digest() {
        let err = format!("docker.io/example/os:latest@sha256:{}", "2".repeat(64))
            .parse::<TargetImage>()
            .unwrap_err();
        assert!(err.to_string().contains("both a tag and a digest"), "{err}");
    }
}

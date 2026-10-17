//! Build a Mender Artifact for the bootc Update Module
//! from a container image.

use std::process::Command;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use oci_spec::distribution::Reference;

use crate::command::CommandRunExt;
use crate::meta_data::{MetaData, TargetImage};

#[derive(Debug, Parser)]
#[command(version)]
pub struct Cli {
    /// The Artifact name.
    #[arg(short = 'n', long)]
    pub artifact_name: String,

    /// A compatible type of the devices to update. May be repeated.
    #[arg(short = 'c', long, value_name = "TYPE", required = true)]
    pub compatible_types: Vec<String>,

    /// Where to write the Artifact.
    #[arg(short = 'o', long, default_value = "bootc-artifact.mender")]
    pub output_path: Utf8PathBuf,

    /// The image to update to, e.g. `docker.io/example/os:latest`. Tags are
    /// resolved to their current digest when the Artifact is built.
    pub image: String,

    /// Options for `mender-artifact write module-image`, e.g. `--key key.pem` to
    /// sign the Artifact.
    #[arg(last = true)]
    pub mender_artifact_args: Vec<String>,
}

impl Cli {
    /// Write the Artifact.
    ///
    /// # Errors
    ///
    /// Fails if the image cannot be pinned or mender-artifact fails.
    pub fn run(self) -> Result<()> {
        let image = pin(&self.image)?;
        tracing::info!("Updating to {image}");
        let meta_data = tempfile::NamedTempFile::new()?;
        serde_json::to_writer(&meta_data, &MetaData { image })?;
        let meta_data = meta_data
            .path()
            .to_str()
            .context("non-UTF-8 temporary path")?;
        Command::new("mender-artifact")
            .args(self.write_args(meta_data))
            .run_inherited()
    }

    /// The arguments of `mender-artifact` for an Artifact with `meta_data`.
    fn write_args<'a>(&'a self, meta_data: &'a str) -> Vec<&'a str> {
        let mut args = vec!["write", "module-image", "--type", "bootc"];
        args.extend(["--artifact-name", &self.artifact_name]);
        for compatible_type in &self.compatible_types {
            args.extend(["--compatible-types", compatible_type]);
        }
        args.extend(["--output-path", self.output_path.as_str()]);
        args.extend(["--meta-data", meta_data]);
        args.extend(self.mender_artifact_args.iter().map(String::as_str));
        args
    }
}

/// Pin `image` by digest.
fn pin(image: &str) -> Result<TargetImage> {
    let reference: Reference = image
        .parse()
        .with_context(|| format!("{image} is not a valid image reference"))?;
    if reference.digest().is_some() {
        return image.parse();
    }
    let manifest = tempfile::NamedTempFile::new()?;
    let output = Command::new("skopeo")
        .args(["inspect", "--raw", &format!("docker://{image}")])
        .run_get_output()?;
    std::fs::write(&manifest, output)?;
    let digest = Command::new("skopeo")
        .arg("manifest-digest")
        .arg(manifest.path())
        .run_get_output()?;
    let digest = String::from_utf8(digest)?;
    format!(
        "{}/{}@{}",
        reference.registry(),
        reference.repository(),
        digest.trim_end()
    )
    .parse()
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn writes_bootc_module_image() {
        let cli = Cli::parse_from([
            "bootc-artifact-gen",
            "--artifact-name",
            "foo",
            "--compatible-types",
            "orin",
            "docker.io/example/os:latest",
            "--",
            "--key",
            "key.pem",
        ]);
        assert_eq!(
            cli.write_args("meta-data.json").join(" "),
            "write module-image --type bootc --artifact-name foo --compatible-types orin \
             --output-path bootc-artifact.mender --meta-data meta-data.json \
             --key key.pem"
        );
    }
}

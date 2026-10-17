//! Developer tasks, run with `cargo xtask`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use xshell::{Shell, cmd};

/// The repository the test images are tagged under.
const REPOSITORY: &str = "localhost/mender-bootc-test";

#[derive(Debug, Parser)]
enum Task {
    /// Build the base and the update test image.
    Images,
}

fn main() -> Result<()> {
    let sh = Shell::new()?;
    sh.change_dir(workspace_dir()?);
    match Task::parse() {
        Task::Images => images(&sh),
    }
}

/// The root of the workspace.
fn workspace_dir() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir
        .parent()
        .context("xtask is not in a workspace")?
        .to_owned())
}

/// Build the images the tests update from and to.
fn images(sh: &Shell) -> Result<()> {
    for version in ["base", "update"] {
        cmd!(
            sh,
            "podman build --build-context source=. --build-arg IMAGE_VERSION={version} --tag {REPOSITORY}:{version} tests/images/fedora"
        )
        .run()?;
    }
    Ok(())
}

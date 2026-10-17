//! Developer tasks, run with `cargo xtask`.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, ensure};
use clap::{Args, Parser};
use xshell::{Shell, cmd};

/// The repository the test images are tagged under.
const REPOSITORY: &str = "localhost/mender-bootc-test";

/// The container of the registry the tests pull the update image from.
const REGISTRY: &str = "mender-bootc-registry";

/// The registry container image.
const REGISTRY_IMAGE: &str = "quay.io/libpod/registry:2.8";

/// Where the host reaches the registry.
const REGISTRY_HOST: &str = "127.0.0.1:5000";

/// Where the test VMs reach the registry, see
/// `tests/images/fedora/system_files/etc/containers/registries.conf.d`.
const REGISTRY_VM: &str = "10.0.2.2:5000";

/// The environment variable setting `Build`.
const BUILD_OPTIONS_ENV: &str = "MENDER_BOOTC_BUILD_OPTIONS";

#[derive(Debug, Parser)]
enum Task {
    /// Build the base and the update test image.
    Images {
        #[command(flatten)]
        build: Build,
    },
    /// Build the test images and run the integration tests against VMs booted
    /// from the base image.
    Test {
        #[command(flatten)]
        build: Build,
        /// Arguments for the test binary, e.g. a test name filter.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
}

fn main() -> Result<()> {
    let sh = Shell::new()?;
    sh.change_dir(workspace_dir()?);
    match Task::parse() {
        Task::Images { build } => images(&sh, &build),
        Task::Test { build, args } => test(&sh, &build, &args),
    }
}

/// How the test images are built.
#[derive(Debug, Args)]
struct Build {
    /// An option passed to `podman build`, such as a cache.
    #[arg(long = "build-option", env = BUILD_OPTIONS_ENV, value_delimiter = ' ', allow_hyphen_values = true)]
    options: Vec<String>,
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
fn images(sh: &Shell, build: &Build) -> Result<()> {
    let options: Vec<&String> = build
        .options
        .iter()
        .filter(|option| !option.is_empty())
        .collect();
    let options = options.as_slice();
    for version in ["base", "update"] {
        cmd!(
            sh,
            "podman build {options...} --build-context source=. --build-arg IMAGE_VERSION={version} --tag {REPOSITORY}:{version} tests/images/fedora"
        )
        .run()?;
    }
    Ok(())
}

/// Push the update image to the registry and run the tests against it.
fn test(sh: &Shell, build: &Build, args: &[OsString]) -> Result<()> {
    // bootc-artifact-gen runs mender-artifact.
    let missing: Vec<_> = ["bcvk", "mender-artifact", "podman"]
        .into_iter()
        .filter(|tool| which::which(tool).is_err())
        .collect();
    ensure!(
        missing.is_empty(),
        "the test suite requires: {}",
        missing.join(", ")
    );
    images(sh, build)?;
    let _registry = Registry::start(sh)?;
    let update = push_update(sh)?;
    cmd!(
        sh,
        "cargo test --locked --features integration --test integration -- --test-threads=1 {args...}"
    )
    .env("MENDER_BOOTC_TEST_UPDATE", update)
    .run()?;
    Ok(())
}

/// The registry the tests pull the update image from, removed when dropped.
#[derive(Debug)]
struct Registry;

impl Registry {
    /// Start the registry, replacing any existing one.
    fn start(sh: &Shell) -> Result<Self> {
        cmd!(
            sh,
            "podman run --detach --rm --replace --name {REGISTRY} --publish {REGISTRY_HOST}:5000 {REGISTRY_IMAGE}"
        )
        .run()?;
        Ok(Self)
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        let _ = Command::new("podman")
            .args(["rm", "--force", REGISTRY])
            .output();
    }
}

/// Push the update image to the registry, returning the reference the VMs
/// pull it by.
fn push_update(sh: &Shell) -> Result<String> {
    let temp_dir = sh.create_temp_dir()?;
    let digest_file = temp_dir.path().join("digest");
    cmd!(
        sh,
        "podman push --tls-verify=false --digestfile {digest_file} {REPOSITORY}:update {REGISTRY_HOST}/mender-bootc-test:update"
    )
    .run()?;
    let digest = sh.read_file(&digest_file)?;
    Ok(format!("{REGISTRY_VM}/mender-bootc-test@{}", digest.trim()))
}

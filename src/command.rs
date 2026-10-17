//! Run external commands.

use std::process::{Command, Stdio};

use anyhow::{Context, Result, ensure};
use serde::de::DeserializeOwned;

/// Run a command to completion.
pub(crate) trait CommandRunExt {
    /// Run the command, inheriting its stdout and stderr.
    fn run_inherited(&mut self) -> Result<()>;

    /// Run the command, returning its stdout. Its stderr is inherited.
    fn run_get_output(&mut self) -> Result<Vec<u8>>;

    /// Run the command and parse its stdout as JSON.
    fn run_and_parse_json<T: DeserializeOwned>(&mut self) -> Result<T>;
}

impl CommandRunExt for Command {
    fn run_inherited(&mut self) -> Result<()> {
        let status = self
            .status()
            .with_context(|| format!("running {}", describe(self)))?;
        ensure!(status.success(), "{} failed with {status}", describe(self));
        Ok(())
    }

    fn run_get_output(&mut self) -> Result<Vec<u8>> {
        let output = self
            .stderr(Stdio::inherit())
            .output()
            .with_context(|| format!("running {}", describe(self)))?;
        ensure!(
            output.status.success(),
            "{} failed with {}",
            describe(self),
            output.status
        );
        Ok(output.stdout)
    }

    fn run_and_parse_json<T: DeserializeOwned>(&mut self) -> Result<T> {
        let output = self.run_get_output()?;
        serde_json::from_slice(&output)
            .with_context(|| format!("parsing the output of {}", describe(self)))
    }
}

/// The command line, e.g. `bootc switch --quiet …`.
fn describe(command: &Command) -> String {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

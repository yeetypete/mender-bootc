//! Mender Update Module CLI. Mender runs it as
//! `/usr/share/mender/modules/v3/bootc <STATE> <FILE-TREE>`.

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;

use crate::bootc::BootcCli;
use crate::module;
use crate::protocol::State;

#[derive(Debug, Parser)]
#[command(version)]
pub struct Cli {
    /// The state or query, e.g. `ArtifactInstall`.
    pub state: State,

    /// The file tree Mender unpacks the Artifact's headers into, e.g.
    /// /var/lib/mender/modules/v3/payloads/0000/tree. The image reference to
    /// update to is in this file's header/meta-data.
    pub file_tree: Utf8PathBuf,
}

impl Cli {
    /// Run the state or respond to the query.
    ///
    /// # Errors
    ///
    /// Returns the state's error, which fails the update.
    pub fn run(self) -> Result<()> {
        module::run(
            &self.state,
            &self.file_tree,
            &BootcCli,
            &mut std::io::stdout().lock(),
        )
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}

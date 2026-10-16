//! The Mender update modules v3 protocol, see
//! <https://github.com/mendersoftware/mender/blob/master/Documentation/update-modules-v3-file-api.md>.

use std::io::Write;

use anyhow::Result;
use strum::{Display, EnumString};

/// The state or query Mender calls the Update Module with.
#[derive(Debug, Clone, PartialEq, Eq, Display, EnumString)]
pub enum State {
    // Regular states, in execution order.
    Download,
    DownloadWithFileSizes,
    ArtifactInstall,
    ArtifactReboot,
    ArtifactVerifyReboot,
    ArtifactCommit,
    Cleanup,
    // Error states.
    ArtifactRollback,
    ArtifactRollbackReboot,
    ArtifactVerifyRollbackReboot,
    ArtifactFailure,
    // Queries.
    ProvidePayloadFileSizes,
    NeedsArtifactReboot,
    SupportsRollback,
    /// A state or query of a later protocol revision, which modules must
    /// ignore.
    #[strum(default)]
    Other(String),
}

/// The response to `NeedsArtifactReboot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum RebootAction {
    No,
    Automatic,
    Yes,
}

/// Write the response to a query.
///
/// # Errors
///
/// Fails if `out` cannot be written.
pub fn respond(out: &mut impl Write, response: impl std::fmt::Display) -> Result<()> {
    out.write_all(format!("{response}\n").as_bytes())?;
    Ok(out.flush()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_states() {
        assert_eq!("ArtifactInstall".parse(), Ok(State::ArtifactInstall));
        assert_eq!(
            "SupportsAugmentedArtifacts".parse(),
            Ok(State::Other("SupportsAugmentedArtifacts".to_owned()))
        );
    }

    #[test]
    fn writes_response_line() -> Result<()> {
        let mut out = Vec::new();
        respond(&mut out, RebootAction::Automatic)?;
        assert_eq!(out, b"Automatic\n");
        Ok(())
    }
}

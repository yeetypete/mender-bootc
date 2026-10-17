use std::io::IsTerminal;
use std::process::ExitCode;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

pub mod artifact_gen;
pub mod bootc;
pub mod cli;
pub mod command;
pub mod meta_data;
pub mod module;
pub mod protocol;

/// Log to stderr at `info` and above.
pub fn initialize_tracing() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .without_time()
        .with_target(false)
        .with_ansi(std::io::stderr().is_terminal())
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();
}

/// Run a binary's `main`, printing its error.
// The final error must reach the user regardless of the `RUST_LOG` filter.
// We bypass tracing and go straight to stderr.
#[allow(clippy::print_stderr)]
pub fn run_main(main: impl FnOnce() -> anyhow::Result<()>) -> ExitCode {
    match main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

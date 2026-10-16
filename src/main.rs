use std::io::IsTerminal;
use std::process::ExitCode;

use clap::Parser;
use mender_bootc::cli::Cli;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

// The final error must reach the user regardless of the `RUST_LOG` filter.
// We bypass tracing and go straight to stderr.
#[allow(clippy::print_stderr)]
fn main() -> ExitCode {
    init_tracing();
    match Cli::parse().run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Log to stderr at `info` and above, which Mender adds to the deployment
/// log.
fn init_tracing() {
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

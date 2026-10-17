use std::process::ExitCode;

use clap::Parser;
use mender_bootc::cli::Cli;

fn main() -> ExitCode {
    mender_bootc::run_main(|| {
        mender_bootc::initialize_tracing();
        Cli::parse().run()
    })
}

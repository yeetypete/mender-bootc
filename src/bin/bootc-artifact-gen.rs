use std::process::ExitCode;

use clap::Parser;
use mender_bootc::artifact_gen::Cli;

fn main() -> ExitCode {
    mender_bootc::run_main(|| {
        mender_bootc::initialize_tracing();
        Cli::parse().run()
    })
}

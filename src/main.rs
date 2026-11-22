//! `pflease`: list and filter pfSense DHCP leases in the terminal.

mod cli;

use std::process::ExitCode;

use clap::Parser;
use jiff::tz::TimeZone;
use pflease::LeaseSource;

/// Why a command could not run.
#[derive(Debug, thiserror::Error)]
enum RunError {
    /// No lease source is implemented yet, so `main` cannot build one.
    #[error("no lease source configured")]
    NoSource,
    /// The source failed to produce its leases.
    #[error(transparent)]
    Source(#[from] pflease::SourceError),
}

fn main() -> ExitCode {
    match run(&cli::Cli::parse(), None, &TimeZone::system()) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("pflease: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Runs `cli` against `source` and returns what to print; times are shown
/// in `local` unless `--utc` is given.
fn run(
    cli: &cli::Cli,
    source: Option<&dyn LeaseSource>,
    local: &TimeZone,
) -> Result<String, RunError> {
    match cli.command {
        cli::Command::List { utc } => {
            let source = source.ok_or(RunError::NoSource)?;
            let tz = if utc { TimeZone::UTC } else { local.clone() };
            Ok(pflease::table::list(source, &tz)?)
        }
    }
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;

//! `pflease`: list and filter pfSense DHCP leases in the terminal.

mod cli;

use std::ffi::OsString;
use std::process::ExitCode;

use clap::Parser;
use jiff::tz::TimeZone;
use pflease::LeaseSource;
use pflease::config::{self, ConfigError};

/// Why a command could not run.
#[derive(Debug, thiserror::Error)]
enum RunError {
    /// The settings could not be loaded.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// No lease source is implemented yet, so `main` cannot build one.
    #[error("no lease source configured")]
    NoSource,
    /// The source failed to produce its leases.
    #[error(transparent)]
    Source(#[from] pflease::SourceError),
}

fn main() -> ExitCode {
    let env = |name: &str| std::env::var_os(name);
    match execute(&cli::Cli::parse(), &env, &TimeZone::system()) {
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

/// Loads the settings (environment read with `env`), then runs `cli`.
fn execute(
    cli: &cli::Cli,
    env: &dyn Fn(&str) -> Option<OsString>,
    local: &TimeZone,
) -> Result<String, RunError> {
    // Checked now so a bad config fails early; the SSH source that consumes
    // the settings is built from them in a later change.
    let _settings = config::load(&cli.config.options(), env)?;
    run(cli, None, local)
}

/// Runs `cli` against `source` and returns what to print; times are shown
/// in `local` unless `--utc` is given.
fn run(
    cli: &cli::Cli,
    source: Option<&dyn LeaseSource>,
    local: &TimeZone,
) -> Result<String, RunError> {
    match &cli.command {
        cli::Command::List { utc, filter } => {
            let source = source.ok_or(RunError::NoSource)?;
            let tz = if *utc { TimeZone::UTC } else { local.clone() };
            Ok(pflease::table::list(source, &filter.into(), &tz)?)
        }
    }
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;

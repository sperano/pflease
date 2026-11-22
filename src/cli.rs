//! Command-line interface definition.

use clap::{Parser, Subcommand};

/// List and filter the DHCP leases of a pfSense firewall.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List the current DHCP leases, ordered by IP address.
    List {
        /// Show start and end times in UTC instead of local time.
        #[arg(long)]
        utc: bool,
    },
}

#[cfg(test)]
#[path = "cli_test.rs"]
mod tests;

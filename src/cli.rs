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
    /// List the current DHCP leases.
    List,
}

#[cfg(test)]
#[path = "cli_test.rs"]
mod tests;

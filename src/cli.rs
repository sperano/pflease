//! Command-line interface definition.

use std::num::NonZeroU16;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use pflease::config::{Layer, Options};

/// List and filter the DHCP leases of a pfSense firewall.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    #[command(flatten)]
    pub config: ConfigArgs,
    #[command(subcommand)]
    pub command: Command,
}

/// Where to find the firewall; each flag overrides the config file and the
/// matching `PFLEASE_*` environment variable.
#[derive(Debug, Default, Args)]
pub struct ConfigArgs {
    /// Config file [default: ~/.config/pflease/config.toml, or under
    /// `XDG_CONFIG_HOME` when set].
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
    /// Named firewall from the config file's [firewalls.<name>] tables.
    #[arg(long, global = true, value_name = "NAME")]
    pub firewall: Option<String>,
    /// SSH destination of the firewall (hostname, address or ssh alias).
    #[arg(long, global = true)]
    pub host: Option<String>,
    /// SSH user.
    #[arg(long, global = true)]
    pub user: Option<String>,
    /// SSH port.
    #[arg(long, global = true)]
    pub port: Option<NonZeroU16>,
    /// Private key file passed to `ssh -i`.
    #[arg(long, global = true, value_name = "PATH")]
    pub identity_file: Option<PathBuf>,
    /// Path of the lease database on the firewall.
    #[arg(long, global = true, value_name = "PATH")]
    pub lease_file: Option<String>,
}

impl ConfigArgs {
    /// The flags as configuration [`Options`].
    pub fn options(&self) -> Options {
        Options {
            config: self.config.clone(),
            firewall: self.firewall.clone(),
            flags: Layer {
                host: self.host.clone(),
                user: self.user.clone(),
                port: self.port,
                identity_file: self.identity_file.clone(),
                lease_file: self.lease_file.clone(),
            },
        }
    }
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

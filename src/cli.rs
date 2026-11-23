//! Command-line interface definition.

use std::num::NonZeroU16;
use std::path::PathBuf;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, Parser, Subcommand};
use pflease::config::{Layer, Options};
use pflease::{Filter, IpRange, LeaseState, MacPrefix};

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
    /// `XDG_CONFIG_HOME` when it is set to an absolute path].
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
                interfaces: None,
            },
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List the current DHCP leases, ordered by IP address.
    ///
    /// Filters combine with AND; repeating a filter accepts any of its
    /// values.
    List {
        /// Show start and end times in UTC instead of local time.
        #[arg(long)]
        utc: bool,
        #[command(flatten)]
        filter: FilterArgs,
    },
}

/// The `list` flags that select leases.
#[derive(Debug, Default, Args)]
pub struct FilterArgs {
    /// Hostname contains PATTERN, ignoring case; leases without a hostname
    /// never match.
    #[arg(long = "hostname", value_name = "PATTERN")]
    pub hostnames: Vec<String>,
    /// MAC address starts with PREFIX (full address or OUI; any of ':', '-',
    /// '.' between digit pairs; any case).
    #[arg(long = "mac", value_name = "PREFIX")]
    pub macs: Vec<MacPrefix>,
    /// Address is RANGE: a CIDR block (10.0.0.0/24), an inclusive range
    /// (10.0.0.10-10.0.0.50) or one address.
    #[arg(long = "ip", value_name = "RANGE")]
    pub ips: Vec<IpRange>,
    /// Lease is in STATE.
    #[arg(
        long = "state",
        value_name = "STATE",
        ignore_case = true,
        value_parser = state_parser(),
    )]
    pub states: Vec<LeaseState>,
    /// Served on interface NAME, matched whole, ignoring case.
    #[arg(long = "interface", value_name = "NAME")]
    pub interfaces: Vec<String>,
}

/// Accepts only the known state names, so clap lists them in `--help` and
/// rejects anything else before it can become [`LeaseState::Other`].
fn state_parser() -> impl TypedValueParser<Value = LeaseState> {
    PossibleValuesParser::new(LeaseState::known_names()).try_map(|name| name.parse::<LeaseState>())
}

impl From<&FilterArgs> for Filter {
    fn from(args: &FilterArgs) -> Self {
        let filter = args
            .hostnames
            .iter()
            .fold(Self::default(), |f, p| f.hostname(p));
        let filter = args.macs.iter().cloned().fold(filter, Self::mac);
        let filter = args.ips.iter().copied().fold(filter, Self::ip);
        let filter = args.states.iter().cloned().fold(filter, Self::state);
        args.interfaces
            .iter()
            .fold(filter, |f, name| f.interface(name))
    }
}

#[cfg(test)]
#[path = "cli_test.rs"]
mod tests;

//! Command-line interface definition.

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, Parser, Subcommand};
use pflease::{Filter, IpRange, LeaseState, MacPrefix};

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

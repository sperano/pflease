use std::net::Ipv4Addr;

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use pflease::{Filter, Lease, LeaseState, MacAddr, Sort, SortKey};

use super::{Cli, Command, FilterArgs, SortArgs};

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("arguments parse")
}

fn list_args(extra: &[&str]) -> FilterArgs {
    let args: Vec<&str> = ["pflease", "list"].iter().chain(extra).copied().collect();
    match parse(&args).command {
        Command::List { filter, .. } => filter,
    }
}

fn sort_args(extra: &[&str]) -> SortArgs {
    let args: Vec<&str> = ["pflease", "list"].iter().chain(extra).copied().collect();
    match parse(&args).command {
        Command::List { sort, .. } => sort,
    }
}

fn error_kind(extra: &[&str]) -> ErrorKind {
    let args: Vec<&str> = ["pflease", "list"].iter().chain(extra).copied().collect();
    Cli::try_parse_from(args)
        .expect_err("arguments rejected")
        .kind()
}

fn error_text(extra: &[&str]) -> String {
    let args: Vec<&str> = ["pflease", "list"].iter().chain(extra).copied().collect();
    Cli::try_parse_from(args)
        .expect_err("arguments rejected")
        .to_string()
}

#[test]
fn cli_definition_is_valid() {
    Cli::command().debug_assert();
}

#[test]
fn parses_list_in_local_time_by_default() {
    let cli = parse(&["pflease", "list"]);
    assert!(matches!(cli.command, Command::List { utc: false, .. }));
}

#[test]
fn parses_list_utc() {
    let cli = parse(&["pflease", "list", "--utc"]);
    assert!(matches!(cli.command, Command::List { utc: true, .. }));
}

#[test]
fn list_without_filters_has_none() {
    let args = list_args(&[]);
    assert_eq!(Filter::from(&args), Filter::default());
}

#[test]
fn parses_repeated_hostname() {
    let args = list_args(&["--hostname", "Desk", "--hostname", "phone"]);
    assert_eq!(args.hostnames, ["Desk", "phone"]);
}

#[test]
fn parses_repeated_mac() {
    let args = list_args(&["--mac", "AA-bb:cc", "--mac", "aa:bb:cc:dd:ee:ff"]);
    let shown: Vec<String> = args.macs.iter().map(ToString::to_string).collect();
    assert_eq!(shown, ["aa:bb:cc", "aa:bb:cc:dd:ee:ff"]);
}

#[test]
fn parses_every_ip_form() {
    let args = list_args(&[
        "--ip",
        "10.0.0.0/24",
        "--ip",
        "10.0.1.10-10.0.1.50",
        "--ip",
        "10.0.2.7",
    ]);
    let shown: Vec<String> = args.ips.iter().map(ToString::to_string).collect();
    assert_eq!(
        shown,
        ["10.0.0.0-10.0.0.255", "10.0.1.10-10.0.1.50", "10.0.2.7"]
    );
}

#[test]
fn parses_states_ignoring_case() {
    let args = list_args(&["--state", "active", "--state", "Expired-Reclaimed"]);
    assert_eq!(
        args.states,
        [LeaseState::Active, LeaseState::ExpiredReclaimed]
    );
}

#[test]
fn parses_repeated_interface() {
    let args = list_args(&["--interface", "LAN", "--interface", "opt1"]);
    assert_eq!(args.interfaces, ["LAN", "opt1"]);
}

#[test]
fn rejects_unknown_state() {
    assert_eq!(error_kind(&["--state", "bogus"]), ErrorKind::InvalidValue);
    assert_eq!(error_kind(&["--state", "default"]), ErrorKind::InvalidValue);
}

#[test]
fn rejects_reversed_ip_range_with_its_reason() {
    assert_eq!(
        error_kind(&["--ip", "10.0.0.9-10.0.0.1"]),
        ErrorKind::ValueValidation
    );
    assert!(
        error_text(&["--ip", "10.0.0.9-10.0.0.1"])
            .contains("range 10.0.0.9-10.0.0.1 is reversed: 10.0.0.9 is after 10.0.0.1"),
    );
}

#[test]
fn rejects_invalid_prefix_length_with_its_reason() {
    assert_eq!(
        error_kind(&["--ip", "10.0.0.0/33"]),
        ErrorKind::ValueValidation
    );
    assert!(
        error_text(&["--ip", "10.0.0.0/33"])
            .contains("CIDR prefix length \"33\" is not a number from 0 to 32"),
    );
}

#[test]
fn rejects_invalid_mac_prefix_with_its_reason() {
    assert_eq!(error_kind(&["--mac", "aab:cc"]), ErrorKind::ValueValidation);
    assert!(
        error_text(&["--mac", "aab:cc"])
            .contains("MAC prefix group \"aab\" is not an even number of hex digits"),
    );
}

fn lease(hostname: &str, state: LeaseState, interface: &str) -> Lease {
    Lease {
        ip: Ipv4Addr::new(10, 0, 0, 5),
        mac: MacAddr::new([0xaa, 0xbb, 0xcc, 0, 0, 5]),
        hostname: Some(hostname.into()),
        start: None,
        end: None,
        state,
        interface: Some(interface.into()),
    }
}

#[test]
fn filter_from_args_carries_every_flag() {
    let filter = Filter::from(&list_args(&[
        "--hostname",
        "nas",
        "--mac",
        "aa:bb:cc",
        "--ip",
        "10.0.0.0/29",
        "--state",
        "free",
        "--state",
        "active",
        "--interface",
        "Opt1",
    ]));
    assert!(filter.matches(&lease("NAS-1", LeaseState::Active, "opt1")));
    assert!(!filter.matches(&lease("printer", LeaseState::Active, "opt1")));
    assert!(!filter.matches(&lease("nas", LeaseState::Expired, "opt1")));
    assert!(!filter.matches(&lease("nas", LeaseState::Active, "lan")));
    let other_mac = Lease {
        mac: MacAddr::new([0xaa, 0xbb, 0xcd, 0, 0, 5]),
        ..lease("nas", LeaseState::Active, "opt1")
    };
    assert!(!filter.matches(&other_mac));
    let other_ip = Lease {
        ip: Ipv4Addr::new(10, 0, 0, 8),
        ..lease("nas", LeaseState::Active, "opt1")
    };
    assert!(!filter.matches(&other_ip));
}

#[test]
fn list_sorts_by_ascending_ip_by_default() {
    assert_eq!(Sort::from(&sort_args(&[])), Sort::default());
}

#[test]
fn parses_sort_key_ignoring_case_and_reverse() {
    let sort = Sort::from(&sort_args(&["--sort", "Hostname", "--reverse"]));
    assert_eq!(
        sort,
        Sort {
            key: SortKey::Hostname,
            reverse: true,
        }
    );
}

#[test]
fn parses_every_sort_key() {
    let keys: Vec<SortKey> = [
        "ip",
        "mac",
        "hostname",
        "start",
        "end",
        "state",
        "interface",
    ]
    .iter()
    .map(|name| sort_args(&["--sort", name]).key)
    .collect();
    assert_eq!(
        keys,
        [
            SortKey::Ip,
            SortKey::Mac,
            SortKey::Hostname,
            SortKey::Start,
            SortKey::End,
            SortKey::State,
            SortKey::Interface,
        ]
    );
}

#[test]
fn rejects_unknown_sort_key() {
    assert_eq!(error_kind(&["--sort", "size"]), ErrorKind::InvalidValue);
}

#[test]
fn help_says_missing_values_sort_last() {
    let help = Cli::command()
        .find_subcommand_mut("list")
        .expect("list subcommand")
        .render_long_help()
        .to_string();
    assert!(help.contains("Leases missing the value sort last, also with --reverse"));
    assert!(help.contains("[default: ip]"));
}

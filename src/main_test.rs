use std::ffi::OsString;
use std::net::Ipv4Addr;

use clap::Parser;
use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};
use pflease::{Lease, LeaseSource, LeaseState, MacAddr, SourceError};

use super::{RunError, cli::Cli, execute, run};

/// The library's `FakeSource` is test-only inside the library, so the binary
/// keeps its own: a source that fails with a fixed message.
struct Failing;

impl LeaseSource for Failing {
    fn leases(&self) -> Result<Vec<Lease>, SourceError> {
        Err(SourceError::Unavailable("connection refused".into()))
    }

    fn static_mappings(&self) -> Result<Vec<Lease>, SourceError> {
        Err(SourceError::Unavailable("connection refused".into()))
    }
}

/// A source with no leases.
struct Empty;

impl LeaseSource for Empty {
    fn leases(&self) -> Result<Vec<Lease>, SourceError> {
        Ok(Vec::new())
    }

    fn static_mappings(&self) -> Result<Vec<Lease>, SourceError> {
        Ok(Vec::new())
    }
}

/// 2023-11-14 22:13:20 UTC.
const START_SECONDS: i64 = 1_700_000_000;
/// Hours east of UTC of the zone standing in for the system zone.
const LOCAL_HOURS: i8 = 2;

/// A source with one lease that has only a start time.
struct Started;

impl LeaseSource for Started {
    fn leases(&self) -> Result<Vec<Lease>, SourceError> {
        Ok(vec![Lease {
            ip: Ipv4Addr::new(10, 0, 0, 1),
            mac: MacAddr::new([0x02, 0, 0, 0, 0, 1]),
            hostname: None,
            start: Some(Timestamp::from_second(START_SECONDS).expect("valid timestamp")),
            end: None,
            state: LeaseState::Active,
            interface: None,
        }])
    }

    fn static_mappings(&self) -> Result<Vec<Lease>, SourceError> {
        Ok(Vec::new())
    }
}

fn local() -> TimeZone {
    TimeZone::fixed(Offset::constant(LOCAL_HOURS))
}

/// The START cell of the single lease row of `output`.
fn start_cell(output: &str) -> String {
    let row: Vec<&str> = output
        .lines()
        .nth(1)
        .expect("one lease row")
        .split_whitespace()
        .collect();
    format!("{} {}", row[3], row[4])
}

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("arguments parse")
}

#[test]
fn list_renders_the_source_table() {
    let output =
        run(&parse(&["pflease", "list", "--utc"]), &Empty, &local()).expect("empty source");
    assert_eq!(output, "IP  MAC  HOSTNAME  START  END  STATE  INTERFACE\n");
}

#[test]
fn list_reports_source_errors() {
    let err = run(&parse(&["pflease", "list"]), &Failing, &local()).expect_err("failing source");
    assert!(matches!(err, RunError::Source(SourceError::Unavailable(_))));
    assert_eq!(
        err.to_string(),
        "lease source unavailable: connection refused"
    );
}

#[test]
fn list_shows_times_in_the_local_zone_by_default() {
    let output = run(&parse(&["pflease", "list"]), &Started, &local()).expect("lease");
    assert_eq!(start_cell(&output), "2023-11-15 00:13:20");
}

#[test]
fn list_utc_shows_times_in_utc() {
    let output = run(&parse(&["pflease", "list", "--utc"]), &Started, &local()).expect("lease");
    assert_eq!(start_cell(&output), "2023-11-14 22:13:20");
}

/// An environment with no variables, so no config file is found.
fn no_env(_: &str) -> Option<OsString> {
    None
}

#[test]
fn execute_stops_on_a_config_error_before_running() {
    let err = execute(&parse(&["pflease", "list"]), &no_env, &local()).expect_err("no host");
    assert!(matches!(err, RunError::Config(_)), "{err:?}");
    assert_eq!(
        err.to_string(),
        "missing setting `host`: set `host` in the config file (--config), the \
         PFLEASE_HOST environment variable, or --host"
    );
}

/// A lease at `10.0.0.<last>` named `name`, in `state`.
fn named(last: u8, name: &str, state: LeaseState) -> Lease {
    Lease {
        ip: Ipv4Addr::new(10, 0, 0, last),
        mac: MacAddr::new([0x02, 0, 0, 0, 0, last]),
        hostname: Some(name.into()),
        start: None,
        end: None,
        state,
        interface: None,
    }
}

/// A source with two leases, `10.0.0.1` named `alpha` and `10.0.0.2` named
/// `beta`, and one static mapping, `10.0.0.3` named `gamma`.
struct Two;

impl LeaseSource for Two {
    fn leases(&self) -> Result<Vec<Lease>, SourceError> {
        Ok(vec![
            named(1, "alpha", LeaseState::Active),
            named(2, "beta", LeaseState::Active),
        ])
    }

    fn static_mappings(&self) -> Result<Vec<Lease>, SourceError> {
        Ok(vec![named(3, "gamma", LeaseState::Static)])
    }
}

/// The hostname column of every lease row of `output`.
fn hostnames(output: &str) -> Vec<&str> {
    output
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().nth(2).expect("hostname column"))
        .collect()
}

#[test]
fn list_applies_the_sort_flags() {
    let output = run(
        &parse(&["pflease", "list", "--sort", "hostname", "--reverse"]),
        &Two,
        &local(),
    )
    .expect("leases");
    assert_eq!(hostnames(&output), ["beta", "alpha"]);
}

#[test]
fn list_applies_the_filter_flags() {
    let output = run(
        &parse(&["pflease", "list", "--ip", "10.0.0.2"]),
        &Two,
        &local(),
    )
    .expect("leases");
    assert_eq!(hostnames(&output), ["beta"]);
}

#[test]
fn list_shows_only_leases_without_all() {
    let output = run(&parse(&["pflease", "list"]), &Two, &local()).expect("leases");
    assert_eq!(hostnames(&output), ["alpha", "beta"]);
}

#[test]
fn list_all_adds_the_static_mappings() {
    let output = run(&parse(&["pflease", "list", "--all"]), &Two, &local()).expect("leases");
    assert_eq!(hostnames(&output), ["alpha", "beta", "gamma"]);
}

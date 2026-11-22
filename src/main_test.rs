use std::net::Ipv4Addr;

use clap::Parser;
use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};
use pflease::{Lease, LeaseSource, LeaseState, MacAddr, SourceError};

use super::{RunError, cli::Cli, run};

/// The library's `FakeSource` is test-only inside the library, so the binary
/// keeps its own: a source that fails with a fixed message.
struct Failing;

impl LeaseSource for Failing {
    fn leases(&self) -> Result<Vec<Lease>, SourceError> {
        Err(SourceError::Unavailable("connection refused".into()))
    }
}

/// A source with no leases.
struct Empty;

impl LeaseSource for Empty {
    fn leases(&self) -> Result<Vec<Lease>, SourceError> {
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
fn list_without_source_fails_with_no_source() {
    let err = run(&parse(&["pflease", "list"]), None, &local()).expect_err("no source");
    assert!(matches!(err, RunError::NoSource));
    assert_eq!(err.to_string(), "no lease source configured");
}

#[test]
fn list_renders_the_source_table() {
    let output = run(
        &parse(&["pflease", "list", "--utc"]),
        Some(&Empty),
        &local(),
    )
    .expect("empty source");
    assert_eq!(output, "IP  MAC  HOSTNAME  START  END  STATE  INTERFACE\n");
}

#[test]
fn list_reports_source_errors() {
    let err =
        run(&parse(&["pflease", "list"]), Some(&Failing), &local()).expect_err("failing source");
    assert!(matches!(err, RunError::Source(SourceError::Unavailable(_))));
    assert_eq!(
        err.to_string(),
        "lease source unavailable: connection refused"
    );
}

#[test]
fn list_shows_times_in_the_local_zone_by_default() {
    let output = run(&parse(&["pflease", "list"]), Some(&Started), &local()).expect("lease");
    assert_eq!(start_cell(&output), "2023-11-15 00:13:20");
}

#[test]
fn list_utc_shows_times_in_utc() {
    let output = run(
        &parse(&["pflease", "list", "--utc"]),
        Some(&Started),
        &local(),
    )
    .expect("lease");
    assert_eq!(start_cell(&output), "2023-11-14 22:13:20");
}

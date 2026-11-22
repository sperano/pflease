use clap::Parser;
use pflease::{Lease, LeaseSource, SourceError};

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

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("arguments parse")
}

#[test]
fn list_without_source_fails_with_no_source() {
    let err = run(&parse(&["pflease", "list"]), None).expect_err("no source");
    assert!(matches!(err, RunError::NoSource));
    assert_eq!(err.to_string(), "no lease source configured");
}

#[test]
fn list_renders_the_source_table() {
    let output = run(&parse(&["pflease", "list", "--utc"]), Some(&Empty)).expect("empty source");
    assert_eq!(output, "IP  MAC  HOSTNAME  START  END  STATE  INTERFACE\n");
}

#[test]
fn list_reports_source_errors() {
    let err = run(&parse(&["pflease", "list"]), Some(&Failing)).expect_err("failing source");
    assert!(matches!(err, RunError::Source(SourceError::Unavailable(_))));
    assert_eq!(
        err.to_string(),
        "lease source unavailable: connection refused"
    );
}

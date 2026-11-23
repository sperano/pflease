use std::net::Ipv4Addr;

use jiff::Timestamp;

use super::{FakeSource, LeaseSource, SourceError};
use crate::{Lease, LeaseState, MacAddr};

const START_SECONDS: i64 = 1_700_000_000;
const END_SECONDS: i64 = 1_700_003_600;

fn sample_leases() -> Vec<Lease> {
    vec![
        Lease {
            ip: Ipv4Addr::new(192, 0, 2, 10),
            mac: MacAddr::new([0x02, 0, 0, 0, 0, 0x01]),
            hostname: Some("printer".into()),
            start: Some(Timestamp::from_second(START_SECONDS).expect("valid timestamp")),
            end: Some(Timestamp::from_second(END_SECONDS).expect("valid timestamp")),
            state: LeaseState::Active,
            interface: Some("lan".into()),
        },
        Lease {
            ip: Ipv4Addr::new(192, 0, 2, 11),
            mac: MacAddr::new([0x02, 0, 0, 0, 0, 0x02]),
            hostname: None,
            start: None,
            end: None,
            state: LeaseState::Free,
            interface: None,
        },
    ]
}

#[test]
fn fake_returns_its_leases_on_every_call() {
    let source = FakeSource::Leases(sample_leases());
    assert_eq!(source.leases().expect("fake succeeds"), sample_leases());
    assert_eq!(source.leases().expect("fake succeeds"), sample_leases());
}

#[test]
fn fake_with_no_leases_returns_empty() {
    assert_eq!(
        FakeSource::Leases(Vec::new())
            .leases()
            .expect("fake succeeds"),
        []
    );
}

#[test]
fn failing_fake_reports_unavailable() {
    let err = FakeSource::Failing("connection refused".into())
        .leases()
        .expect_err("failing fake fails");
    assert!(matches!(&err, SourceError::Unavailable(m) if m == "connection refused"));
    assert_eq!(
        err.to_string(),
        "lease source unavailable: connection refused"
    );
}

#[test]
fn fake_with_only_leases_has_no_static_mappings() {
    let source = FakeSource::Leases(sample_leases());
    assert_eq!(source.static_mappings().expect("fake succeeds"), []);
}

#[test]
fn mapped_fake_returns_its_leases_and_mappings() {
    let [lease, mapping] = sample_leases().try_into().expect("two leases");
    let source = FakeSource::Mapped {
        leases: vec![lease.clone()],
        mappings: vec![mapping.clone()],
    };
    assert_eq!(source.leases().expect("fake succeeds"), [lease]);
    assert_eq!(source.static_mappings().expect("fake succeeds"), [mapping]);
}

#[test]
fn failing_fake_fails_static_mappings_too() {
    let err = FakeSource::Failing("connection refused".into())
        .static_mappings()
        .expect_err("failing fake fails");
    assert!(matches!(&err, SourceError::Unavailable(m) if m == "connection refused"));
}

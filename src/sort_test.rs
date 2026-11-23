use std::net::Ipv4Addr;

use jiff::Timestamp;

use super::{ParseSortKeyError, Sort, SortKey};
use crate::{Lease, LeaseState, MacAddr};

/// 2023-11-14 22:13:20 UTC.
const BASE_SECONDS: i64 = 1_700_000_000;

fn at(offset_seconds: i64) -> Timestamp {
    Timestamp::from_second(BASE_SECONDS + offset_seconds).expect("valid timestamp")
}

/// A lease at `10.0.0.<last_octet>` with every optional value missing.
fn lease(last_octet: u8) -> Lease {
    Lease {
        ip: Ipv4Addr::new(10, 0, 0, last_octet),
        mac: MacAddr::new([0x02, 0, 0, 0, 0, last_octet]),
        hostname: None,
        start: None,
        end: None,
        state: LeaseState::Active,
        interface: None,
    }
}

/// The last IP octet of each lease of `leases` once sorted by `key`.
fn order(mut leases: Vec<Lease>, key: SortKey, reverse: bool) -> Vec<u8> {
    Sort { key, reverse }.apply(&mut leases);
    leases.iter().map(|lease| lease.ip.octets()[3]).collect()
}

fn named(last_octet: u8, hostname: Option<&str>) -> Lease {
    Lease {
        hostname: hostname.map(str::to_owned),
        ..lease(last_octet)
    }
}

fn on(last_octet: u8, interface: Option<&str>) -> Lease {
    Lease {
        interface: interface.map(str::to_owned),
        ..lease(last_octet)
    }
}

fn started(last_octet: u8, start: Option<Timestamp>) -> Lease {
    Lease {
        start,
        ..lease(last_octet)
    }
}

fn ending(last_octet: u8, end: Option<Timestamp>) -> Lease {
    Lease {
        end,
        ..lease(last_octet)
    }
}

fn in_state(last_octet: u8, state: &str) -> Lease {
    Lease {
        state: state.parse().expect("non-empty state"),
        ..lease(last_octet)
    }
}

#[test]
fn default_sort_is_ascending_ip() {
    assert_eq!(
        Sort::default(),
        Sort {
            key: SortKey::Ip,
            reverse: false
        }
    );
}

#[test]
fn ip_sorts_numerically() {
    let leases = vec![lease(100), lease(9), lease(10)];
    assert_eq!(order(leases.clone(), SortKey::Ip, false), [9, 10, 100]);
    assert_eq!(order(leases, SortKey::Ip, true), [100, 10, 9]);
}

#[test]
fn mac_sorts_by_bytes() {
    let mac = |last_octet: u8, bytes: [u8; 6]| Lease {
        mac: MacAddr::new(bytes),
        ..lease(last_octet)
    };
    let leases = vec![
        mac(1, [0xff, 0, 0, 0, 0, 0]),
        mac(2, [0x0a, 0, 0, 0, 0, 1]),
        mac(3, [0x0a, 0, 0, 0, 0, 0]),
    ];
    assert_eq!(order(leases.clone(), SortKey::Mac, false), [3, 2, 1]);
    assert_eq!(order(leases, SortKey::Mac, true), [1, 2, 3]);
}

#[test]
fn hostname_sorts_ignoring_case() {
    let leases = vec![
        named(1, Some("charlie")),
        named(2, Some("Bravo")),
        named(3, Some("alpha")),
    ];
    assert_eq!(order(leases.clone(), SortKey::Hostname, false), [3, 2, 1]);
    assert_eq!(order(leases, SortKey::Hostname, true), [1, 2, 3]);
}

#[test]
fn missing_and_empty_hostnames_sort_last_in_both_directions() {
    let leases = vec![
        named(1, None),
        named(2, Some("beta")),
        named(3, Some("")),
        named(4, Some("alpha")),
    ];
    assert_eq!(
        order(leases.clone(), SortKey::Hostname, false),
        [4, 2, 1, 3]
    );
    assert_eq!(order(leases, SortKey::Hostname, true), [2, 4, 1, 3]);
}

#[test]
fn start_sorts_chronologically_missing_last() {
    let leases = vec![
        started(1, Some(at(60))),
        started(2, None),
        started(3, Some(at(0))),
    ];
    assert_eq!(order(leases.clone(), SortKey::Start, false), [3, 1, 2]);
    assert_eq!(order(leases, SortKey::Start, true), [1, 3, 2]);
}

#[test]
fn end_sorts_chronologically_missing_last() {
    let leases = vec![
        ending(1, None),
        ending(2, Some(at(0))),
        ending(3, Some(at(60))),
    ];
    assert_eq!(order(leases.clone(), SortKey::End, false), [2, 3, 1]);
    assert_eq!(order(leases, SortKey::End, true), [3, 2, 1]);
}

#[test]
fn state_sorts_by_name_ignoring_case() {
    let leases = vec![
        in_state(1, "free"),
        in_state(2, "Weird"),
        in_state(3, "active"),
        in_state(4, "expired"),
    ];
    assert_eq!(order(leases.clone(), SortKey::State, false), [3, 4, 1, 2]);
    assert_eq!(order(leases, SortKey::State, true), [2, 1, 4, 3]);
}

#[test]
fn interface_sorts_ignoring_case_missing_last() {
    let leases = vec![
        on(1, None),
        on(2, Some("opt1")),
        on(3, Some("LAN")),
        on(4, Some("")),
    ];
    assert_eq!(
        order(leases.clone(), SortKey::Interface, false),
        [3, 2, 1, 4]
    );
    assert_eq!(order(leases, SortKey::Interface, true), [2, 3, 1, 4]);
}

#[test]
fn equal_values_follow_ascending_ip_in_both_directions() {
    let leases = vec![
        named(30, Some("same")),
        named(2, Some("SAME")),
        named(7, Some("other")),
        named(9, Some("same")),
    ];
    assert_eq!(
        order(leases.clone(), SortKey::Hostname, false),
        [7, 2, 9, 30]
    );
    assert_eq!(order(leases, SortKey::Hostname, true), [2, 9, 30, 7]);
}

#[test]
fn missing_values_follow_ascending_ip_in_both_directions() {
    let leases = vec![named(30, None), named(5, Some("a")), named(2, None)];
    assert_eq!(order(leases.clone(), SortKey::Hostname, false), [5, 2, 30]);
    assert_eq!(order(leases, SortKey::Hostname, true), [5, 2, 30]);
}

#[test]
fn leases_equal_in_key_and_ip_keep_their_source_order() {
    let leases = vec![in_state(1, "free"), in_state(1, "active")];
    let mut sorted = leases.clone();
    Sort {
        key: SortKey::Hostname,
        reverse: true,
    }
    .apply(&mut sorted);
    assert_eq!(sorted, leases);
}

#[test]
fn parses_every_key_name_ignoring_case() {
    let parsed: Vec<SortKey> = [
        "IP",
        "mac",
        "Hostname",
        "start",
        "END",
        "state",
        " interface ",
    ]
    .iter()
    .map(|name| name.parse().expect("known key"))
    .collect();
    assert_eq!(
        parsed,
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
fn rejects_unknown_key_with_its_name() {
    let err = "size".parse::<SortKey>().expect_err("unknown key");
    assert_eq!(err, ParseSortKeyError::Unknown("size".into()));
    assert_eq!(err.to_string(), "unknown sort key \"size\"");
}

#[test]
fn names_are_the_display_names_in_column_order() {
    let names: Vec<&str> = SortKey::names().collect();
    assert_eq!(
        names,
        [
            "ip",
            "mac",
            "hostname",
            "start",
            "end",
            "state",
            "interface"
        ]
    );
    assert_eq!(SortKey::Interface.to_string(), "interface");
}

use std::net::Ipv4Addr;

use super::Filter;
use crate::{Lease, LeaseState, MacAddr};

/// A lease at `10.0.0.<last_octet>` on `lan`, active, named `host-<n>`.
fn lease(last_octet: u8) -> Lease {
    Lease {
        ip: Ipv4Addr::new(10, 0, 0, last_octet),
        mac: MacAddr::new([0xaa, 0xbb, 0xcc, 0, 0, last_octet]),
        hostname: Some(format!("host-{last_octet}")),
        start: None,
        end: None,
        state: LeaseState::Active,
        interface: Some("lan".into()),
    }
}

fn named(hostname: Option<&str>) -> Lease {
    Lease {
        hostname: hostname.map(str::to_owned),
        ..lease(1)
    }
}

fn on(interface: Option<&str>) -> Lease {
    Lease {
        interface: interface.map(str::to_owned),
        ..lease(1)
    }
}

fn ip(text: &str) -> Filter {
    Filter::default().ip(text.parse().expect("valid range"))
}

fn mac(text: &str) -> Filter {
    Filter::default().mac(text.parse().expect("valid prefix"))
}

#[test]
fn default_filter_accepts_every_lease() {
    let bare = Lease {
        hostname: None,
        interface: None,
        state: LeaseState::Free,
        ..lease(1)
    };
    assert!(Filter::default().matches(&bare));
    assert!(Filter::default().matches(&lease(2)));
}

#[test]
fn hostname_matches_substring_ignoring_case() {
    let filter = Filter::default().hostname("ToP");
    assert!(filter.matches(&named(Some("Desktop-1"))));
    assert!(filter.matches(&named(Some("laptop"))));
    assert!(!filter.matches(&named(Some("phone"))));
}

#[test]
fn hostname_ignores_case_beyond_ascii() {
    assert!(
        Filter::default()
            .hostname("ÉTÉ")
            .matches(&named(Some("été-pc")))
    );
}

#[test]
fn missing_or_empty_hostname_never_matches() {
    let filter = Filter::default().hostname("");
    assert!(filter.matches(&named(Some("any"))));
    assert!(!filter.matches(&named(None)));
    assert!(!filter.matches(&named(Some(""))));
}

#[test]
fn mac_prefix_with_mixed_separators_matches() {
    let filter = mac("AA-bb:cc");
    assert!(filter.matches(&lease(1)));
    let other = Lease {
        mac: MacAddr::new([0xaa, 0xbb, 0xcd, 0, 0, 1]),
        ..lease(1)
    };
    assert!(!filter.matches(&other));
}

#[test]
fn full_mac_matches_only_that_address() {
    let filter = mac("aa:bb:cc:00:00:02");
    assert!(filter.matches(&lease(2)));
    assert!(!filter.matches(&lease(3)));
}

#[test]
fn ip_range_includes_its_endpoints() {
    let filter = ip("10.0.0.2-10.0.0.4");
    assert!(!filter.matches(&lease(1)));
    assert!(filter.matches(&lease(2)));
    assert!(filter.matches(&lease(4)));
    assert!(!filter.matches(&lease(5)));
}

#[test]
fn ip_cidr_zero_matches_everything_and_thirty_two_one_address() {
    assert!(ip("192.168.0.0/0").matches(&lease(1)));
    assert!(ip("10.0.0.3/32").matches(&lease(3)));
    assert!(!ip("10.0.0.3/32").matches(&lease(4)));
}

#[test]
fn state_matches_exactly() {
    let filter = Filter::default().state(LeaseState::Expired);
    let expired = Lease {
        state: LeaseState::Expired,
        ..lease(1)
    };
    let reclaimed = Lease {
        state: LeaseState::ExpiredReclaimed,
        ..lease(1)
    };
    assert!(filter.matches(&expired));
    assert!(!filter.matches(&reclaimed));
    assert!(!filter.matches(&lease(1)));
}

#[test]
fn interface_matches_whole_name_ignoring_case() {
    let filter = Filter::default().interface("LAN");
    assert!(filter.matches(&on(Some("lan"))));
    assert!(filter.matches(&on(Some("Lan"))));
    assert!(!filter.matches(&on(Some("lan2"))));
    assert!(!filter.matches(&on(Some("la"))));
}

#[test]
fn missing_or_empty_interface_never_matches() {
    let filter = Filter::default().interface("");
    assert!(!filter.matches(&on(None)));
    assert!(!filter.matches(&on(Some(""))));
}

#[test]
fn repeated_criterion_accepts_any_of_its_values() {
    let filter = Filter::default().hostname("host-1").hostname("host-3");
    assert!(filter.matches(&lease(1)));
    assert!(!filter.matches(&lease(2)));
    assert!(filter.matches(&lease(3)));
}

#[test]
fn repeated_ip_and_state_accept_any_of_their_values() {
    let filter = ip("10.0.0.1").ip("10.0.0.3".parse().expect("valid"));
    assert!(filter.matches(&lease(1)));
    assert!(!filter.matches(&lease(2)));
    assert!(filter.matches(&lease(3)));
    let states = Filter::default()
        .state(LeaseState::Free)
        .state(LeaseState::Active);
    assert!(states.matches(&lease(1)));
}

#[test]
fn different_criteria_must_all_match() {
    let filter = ip("10.0.0.0/24").hostname("host-2").interface("lan");
    assert!(filter.matches(&lease(2)));
    assert!(!filter.matches(&lease(1)), "hostname fails");
    let elsewhere = Lease {
        interface: Some("opt1".into()),
        ..lease(2)
    };
    assert!(!filter.matches(&elsewhere), "interface fails");
    let outside = Lease {
        ip: Ipv4Addr::new(10, 0, 1, 2),
        ..lease(2)
    };
    assert!(!filter.matches(&outside), "address fails");
}

#[test]
fn and_of_ors_needs_one_value_of_each_criterion() {
    let filter = Filter::default()
        .hostname("host-1")
        .hostname("host-2")
        .state(LeaseState::Free)
        .state(LeaseState::Active)
        .mac("aa:bb:cc".parse().expect("valid"))
        .interface("wan")
        .interface("lan");
    assert!(filter.matches(&lease(1)));
    assert!(filter.matches(&lease(2)));
    assert!(!filter.matches(&lease(3)));
    let released = Lease {
        state: LeaseState::Released,
        ..lease(1)
    };
    assert!(!filter.matches(&released));
}

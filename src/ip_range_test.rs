use std::net::Ipv4Addr;

use super::{IpRange, ParseIpRangeError};

fn range(text: &str) -> IpRange {
    text.parse().expect("valid range")
}

fn error(text: &str) -> ParseIpRangeError {
    text.parse::<IpRange>().expect_err("invalid range")
}

fn ends(text: &str) -> (Ipv4Addr, Ipv4Addr) {
    let parsed = range(text);
    (parsed.first(), parsed.last())
}

#[test]
fn cidr_covers_its_network() {
    assert_eq!(
        ends("10.0.0.0/24"),
        (Ipv4Addr::new(10, 0, 0, 0), Ipv4Addr::new(10, 0, 0, 255))
    );
}

#[test]
fn cidr_zero_covers_every_address() {
    assert_eq!(
        ends("10.1.2.3/0"),
        (Ipv4Addr::UNSPECIFIED, Ipv4Addr::BROADCAST)
    );
}

#[test]
fn cidr_thirty_two_is_one_address() {
    let block = range("10.0.0.7/32");
    assert_eq!(
        (block.first(), block.last()),
        (Ipv4Addr::new(10, 0, 0, 7), Ipv4Addr::new(10, 0, 0, 7))
    );
    assert!(block.contains(Ipv4Addr::new(10, 0, 0, 7)));
    assert!(!block.contains(Ipv4Addr::new(10, 0, 0, 6)));
    assert!(!block.contains(Ipv4Addr::new(10, 0, 0, 8)));
}

#[test]
fn cidr_with_host_bits_covers_the_whole_network() {
    assert_eq!(range("10.0.0.77/24"), range("10.0.0.0/24"));
}

#[test]
fn cidr_contains_both_edges_and_nothing_outside() {
    let block = range("10.0.1.0/24");
    assert!(block.contains(Ipv4Addr::new(10, 0, 1, 0)));
    assert!(block.contains(Ipv4Addr::new(10, 0, 1, 255)));
    assert!(!block.contains(Ipv4Addr::new(10, 0, 0, 255)));
    assert!(!block.contains(Ipv4Addr::new(10, 0, 2, 0)));
}

#[test]
fn explicit_range_includes_both_endpoints() {
    let span = range("10.0.0.10-10.0.0.50");
    assert!(span.contains(Ipv4Addr::new(10, 0, 0, 10)));
    assert!(span.contains(Ipv4Addr::new(10, 0, 0, 50)));
    assert!(!span.contains(Ipv4Addr::new(10, 0, 0, 9)));
    assert!(!span.contains(Ipv4Addr::new(10, 0, 0, 51)));
}

#[test]
fn range_of_one_address_is_allowed() {
    assert_eq!(range("10.0.0.5-10.0.0.5"), range("10.0.0.5"));
}

#[test]
fn single_address_matches_only_itself() {
    let one = range("192.168.1.20");
    assert!(one.contains(Ipv4Addr::new(192, 168, 1, 20)));
    assert!(!one.contains(Ipv4Addr::new(192, 168, 1, 21)));
}

#[test]
fn rejects_reversed_range() {
    let err = error("10.0.0.50-10.0.0.10");
    assert_eq!(
        err,
        ParseIpRangeError::Reversed {
            first: Ipv4Addr::new(10, 0, 0, 50),
            last: Ipv4Addr::new(10, 0, 0, 10),
        }
    );
    assert_eq!(
        err.to_string(),
        "range 10.0.0.50-10.0.0.10 is reversed: 10.0.0.50 is after 10.0.0.10"
    );
}

#[test]
fn rejects_prefix_length_over_thirty_two() {
    let err = error("10.0.0.0/33");
    assert_eq!(err, ParseIpRangeError::PrefixLength("33".into()));
    assert_eq!(
        err.to_string(),
        "CIDR prefix length \"33\" is not a number from 0 to 32"
    );
}

#[test]
fn rejects_malformed_prefix_lengths() {
    for length in ["", "+8", "-1", "abc", "256", "24/8"] {
        assert_eq!(
            error(&format!("10.0.0.0/{length}")),
            ParseIpRangeError::PrefixLength(length.into()),
            "{length:?}"
        );
    }
}

#[test]
fn rejects_invalid_addresses() {
    assert_eq!(error("10.0.0"), ParseIpRangeError::Address("10.0.0".into()));
    assert_eq!(
        error("10.0.0.256/24"),
        ParseIpRangeError::Address("10.0.0.256".into())
    );
    assert_eq!(
        error("10.0.0.1-"),
        ParseIpRangeError::Address(String::new())
    );
    assert_eq!(error("::1"), ParseIpRangeError::Address("::1".into()));
    assert_eq!(error("nope").to_string(), "\"nope\" is not an IPv4 address");
}

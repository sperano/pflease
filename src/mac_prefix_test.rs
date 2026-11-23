use super::{MacPrefix, ParseMacPrefixError};
use crate::MacAddr;

const ADDR: MacAddr = MacAddr::new([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);

fn prefix(text: &str) -> MacPrefix {
    text.parse().expect("valid prefix")
}

fn error(text: &str) -> ParseMacPrefixError {
    text.parse::<MacPrefix>().expect_err("invalid prefix")
}

#[test]
fn full_address_matches_only_itself() {
    let full = prefix("aa:bb:cc:dd:ee:ff");
    assert!(full.matches(ADDR));
    assert!(!full.matches(MacAddr::new([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xfe])));
}

#[test]
fn oui_matches_addresses_that_start_with_it() {
    let oui = prefix("aa:bb:cc");
    assert!(oui.matches(ADDR));
    assert!(!oui.matches(MacAddr::new([0xaa, 0xbb, 0xcd, 0xdd, 0xee, 0xff])));
}

#[test]
fn mixed_separators_and_case_parse_to_the_same_prefix() {
    let expected = prefix("aa:bb:cc:dd");
    for text in ["AA-bb:Cc.DD", "aabb.ccdd", "aaBBccDD", "aa-bbcc:dd"] {
        assert_eq!(prefix(text), expected, "{text:?}");
    }
    assert_eq!(expected.to_string(), "aa:bb:cc:dd");
}

#[test]
fn single_octet_prefix_matches() {
    assert!(prefix("AA").matches(ADDR));
    assert!(!prefix("ab").matches(ADDR));
}

#[test]
fn rejects_empty_input() {
    assert_eq!(error(""), ParseMacPrefixError::Empty);
}

#[test]
fn rejects_empty_groups() {
    assert_eq!(error("aa::bb"), ParseMacPrefixError::Group(String::new()));
    assert_eq!(error("aa:"), ParseMacPrefixError::Group(String::new()));
    assert_eq!(error(":"), ParseMacPrefixError::Group(String::new()));
}

#[test]
fn rejects_odd_length_groups() {
    assert_eq!(error("a:bb"), ParseMacPrefixError::Group("a".into()));
    assert_eq!(error("aab:bcc"), ParseMacPrefixError::Group("aab".into()));
}

#[test]
fn rejects_non_hex_digits_and_signs() {
    assert_eq!(error("zz:bb"), ParseMacPrefixError::Group("zz".into()));
    assert_eq!(error("+a:bb"), ParseMacPrefixError::Group("+a".into()));
    assert_eq!(error("aa bb"), ParseMacPrefixError::Group("aa bb".into()));
}

#[test]
fn rejects_non_ascii_groups() {
    assert_eq!(error("aé"), ParseMacPrefixError::Group("aé".into()));
}

#[test]
fn rejects_more_than_six_octets() {
    assert_eq!(
        error("aa:bb:cc:dd:ee:ff:00"),
        ParseMacPrefixError::TooLong(7)
    );
}

#[test]
fn error_messages_name_the_problem() {
    assert_eq!(error("").to_string(), "MAC prefix must not be empty");
    assert_eq!(
        error("a").to_string(),
        "MAC prefix group \"a\" is not an even number of hex digits"
    );
    assert_eq!(
        error("aabbccddeeff00").to_string(),
        "MAC prefix has 7 octets, at most 6 allowed"
    );
}

use std::net::Ipv4Addr;

use jiff::Timestamp;

use super::{ErrorKind, parse, parse_bytes};
use crate::test_support::{DHCPD_LEASES, fixture_leases, fixture_subnets};
use crate::{Lease, LeaseState, MacAddr, Subnets};

/// A lease block for `192.0.2.1` holding `statements`.
fn block(statements: &str) -> String {
    format!("lease 192.0.2.1 {{\n{statements}\n}}\n")
}

/// A block with a MAC, a binding state and `statements`.
fn lease_with(statements: &str) -> String {
    block(&format!(
        "  binding state active;\n  hardware ethernet 02:00:00:00:00:01;\n{statements}"
    ))
}

fn only_lease(text: &str) -> Lease {
    let mut leases = parse(text, &Subnets::default()).expect("parses");
    assert_eq!(leases.len(), 1, "{leases:?}");
    leases.remove(0)
}

/// The line and kind of the error `text` fails with.
fn error(text: &str) -> (usize, ErrorKind) {
    let err = parse(text, &Subnets::default()).expect_err("malformed");
    (err.line(), err.kind().clone())
}

fn value(field: &'static str, value: &str) -> ErrorKind {
    ErrorKind::Value {
        field,
        value: value.to_owned(),
    }
}

fn second(seconds: i64) -> Timestamp {
    Timestamp::from_second(seconds).expect("valid timestamp")
}

#[test]
fn fixture_parses_to_its_current_leases() {
    assert_eq!(
        parse(DHCPD_LEASES, &fixture_subnets()).expect("parses"),
        fixture_leases()
    );
}

#[test]
fn fixture_without_subnets_has_no_interfaces() {
    let leases = parse(DHCPD_LEASES, &Subnets::default()).expect("parses");
    assert!(
        leases.iter().all(|lease| lease.interface.is_none()),
        "{leases:?}"
    );
}

#[test]
fn every_isc_binding_state_maps_to_its_variant() {
    let cases = [
        ("active", LeaseState::Active),
        ("free", LeaseState::Free),
        ("expired", LeaseState::Expired),
        ("released", LeaseState::Released),
        ("abandoned", LeaseState::Abandoned),
        ("reset", LeaseState::Reset),
        ("backup", LeaseState::Backup),
        ("bootp", LeaseState::Bootp),
        ("reserved", LeaseState::Reserved),
    ];
    for (name, state) in cases {
        let text = block(&format!(
            "  binding state {name};\n  hardware ethernet 02:00:00:00:00:01;"
        ));
        assert_eq!(only_lease(&text).state, state, "{name}");
    }
}

#[test]
fn unknown_binding_state_is_kept_as_other() {
    let text = block("  binding state Paused;\n  hardware ethernet 02:00:00:00:00:01;");
    let state = only_lease(&text).state;
    assert!(
        matches!(&state, LeaseState::Other(name) if name.as_str() == "Paused"),
        "{state:?}"
    );
}

#[test]
fn last_block_for_an_address_wins_even_with_older_times() {
    let text = format!(
        "{}{}",
        lease_with("  starts epoch 200;"),
        block("  starts epoch 100;\n  binding state free;\n  hardware ethernet 02:00:00:00:00:02;"),
    );
    let lease = only_lease(&text);
    assert_eq!(lease.state, LeaseState::Free);
    assert_eq!(lease.mac, MacAddr::new([2, 0, 0, 0, 0, 2]));
    assert_eq!(lease.start, Some(second(100)));
}

#[test]
fn later_block_without_hardware_removes_the_earlier_lease() {
    let text = format!("{}{}", lease_with(""), block("  binding state abandoned;"));
    assert_eq!(parse(&text, &Subnets::default()).expect("parses"), []);
}

#[test]
fn leases_keep_the_order_their_addresses_first_appear() {
    let text = "lease 192.0.2.9 { binding state active; hardware ethernet 02:00:00:00:00:09; }\n\
                lease 192.0.2.1 { binding state active; hardware ethernet 02:00:00:00:00:01; }\n\
                lease 192.0.2.9 { binding state free; hardware ethernet 02:00:00:00:00:09; }\n";
    let leases = parse(text, &Subnets::default()).expect("parses");
    let ips: Vec<Ipv4Addr> = leases.iter().map(|lease| lease.ip).collect();
    assert_eq!(
        ips,
        [Ipv4Addr::new(192, 0, 2, 9), Ipv4Addr::new(192, 0, 2, 1)]
    );
    assert_eq!(leases[0].state, LeaseState::Free);
}

#[test]
fn hardwareless_first_block_keeps_its_place_in_the_order() {
    let text = "lease 192.0.2.9 { binding state abandoned; }\n\
                lease 192.0.2.1 { binding state active; hardware ethernet 02:00:00:00:00:01; }\n\
                lease 192.0.2.9 { binding state active; hardware ethernet 02:00:00:00:00:09; }\n";
    let ips: Vec<Ipv4Addr> = parse(text, &Subnets::default())
        .expect("parses")
        .iter()
        .map(|lease| lease.ip)
        .collect();
    assert_eq!(
        ips,
        [Ipv4Addr::new(192, 0, 2, 9), Ipv4Addr::new(192, 0, 2, 1)]
    );
}

#[test]
fn epoch_times_and_their_comment_are_read() {
    let lease = only_lease(&lease_with(
        "  starts epoch 1700000000; # Tue Nov 14 22:13:20 2023\n  ends epoch 1700003600;",
    ));
    assert_eq!(lease.start, Some(second(1_700_000_000)));
    assert_eq!(lease.end, Some(second(1_700_003_600)));
}

#[test]
fn never_is_no_time() {
    let lease = only_lease(&lease_with("  starts never;\n  ends never;"));
    assert_eq!((lease.start, lease.end), (None, None));
}

#[test]
fn single_digit_mac_octets_are_read() {
    let text = block("  binding state active;\n  hardware ethernet 2:0:a:BB:0:1;");
    assert_eq!(
        only_lease(&text).mac,
        MacAddr::new([2, 0, 0x0a, 0xbb, 0, 1])
    );
}

#[test]
fn empty_hostname_is_none_and_escapes_are_decoded() {
    assert_eq!(
        only_lease(&lease_with("  client-hostname \"\";")).hostname,
        None
    );
    let lease = only_lease(&lease_with(r#"  client-hostname "a\"b\\c\101";"#));
    assert_eq!(lease.hostname.as_deref(), Some("a\"b\\cA"));
}

#[test]
fn nested_blocks_and_other_statements_are_ignored() {
    let text = format!(
        "failover peer \"fw\" state {{ my state normal; partner state {{ x; }} }}\n{}",
        lease_with(
            "  on expiry { binding state free; hardware ethernet 02:00:00:00:00:99; \
             set x = \"}\"; }\n  tstp 0 2026/11/22 00:00:00;\n  ;"
        )
    );
    assert_eq!(
        only_lease(&text),
        Lease {
            ip: Ipv4Addr::new(192, 0, 2, 1),
            mac: MacAddr::new([2, 0, 0, 0, 0, 1]),
            hostname: None,
            start: None,
            end: None,
            state: LeaseState::Active,
            interface: None,
        }
    );
}

#[test]
fn statement_missing_its_semicolon_before_a_lease_is_an_error() {
    let text = format!("authoring-byte-order little-endian\n{}", lease_with(""));
    assert_eq!(error(&text), (1, ErrorKind::Unterminated));
}

#[test]
fn block_header_naming_lease_without_an_address_is_skipped() {
    let text = format!(
        "failover peer \"lease\" state {{ my state normal; }}\n{}",
        lease_with("")
    );
    assert_eq!(only_lease(&text).ip, Ipv4Addr::new(192, 0, 2, 1));
}

#[test]
fn malformed_values_name_their_field_and_line() {
    let cases = [
        (
            "  starts 1 2026/13/01 00:00:00;",
            value("starts", "1 2026/13/01 00:00:00"),
        ),
        ("  ends 1 2026/11/01;", value("ends", "1 2026/11/01")),
        (
            "  ends 12 2026/11/01 00:00:00;",
            value("ends", "12 2026/11/01 00:00:00"),
        ),
        ("  starts epoch soon;", value("starts", "epoch soon")),
        (
            "  starts epoch 99999999999999999;",
            value("starts", "epoch 99999999999999999"),
        ),
        (
            "  binding state free now;",
            value("binding state", "free now"),
        ),
        ("  binding state;", value("binding state", "")),
        (
            "  hardware ethernet 02:00:00:00:00;",
            value("hardware ethernet", "02:00:00:00:00"),
        ),
        (
            "  hardware ethernet 02:00:00:00:00:001;",
            value("hardware ethernet", "02:00:00:00:00:001"),
        ),
        (
            "  hardware ethernet 02:00:00:00:00:01:02;",
            value("hardware ethernet", "02:00:00:00:00:01:02"),
        ),
        (
            "  hardware ethernet 02:00:00:00:00:+1;",
            value("hardware ethernet", "02:00:00:00:00:+1"),
        ),
        ("  hardware ethernet;", value("hardware", "ethernet")),
        (
            "  hardware token-ring 02:00:00:00:00:01;",
            ErrorKind::Hardware("token-ring".into()),
        ),
        (
            "  client-hostname \"a\" \"b\";",
            value("client-hostname", "a b"),
        ),
    ];
    for (statement, kind) in cases {
        let text = lease_with(statement);
        assert_eq!(error(&text), (4, kind), "{statement}");
    }
}

#[test]
fn malformed_structure_is_an_error_on_its_line() {
    let cases: [(&str, usize, ErrorKind); 9] = [
        (
            "lease 192.0.2.1 {\n  binding state active;\n",
            1,
            ErrorKind::UnclosedBlock,
        ),
        ("host x {\n  dynamic;\n", 1, ErrorKind::UnclosedBlock),
        ("server-duid \"x\";\n}\n", 2, ErrorKind::UnexpectedClose),
        (
            "authoring-byte-order little-endian\n",
            1,
            ErrorKind::Unterminated,
        ),
        (
            "lease 192.0.2.1 {\n  binding state active\n}\n",
            2,
            ErrorKind::Unterminated,
        ),
        ("lease {\n}\n", 1, ErrorKind::LeaseHeader),
        ("lease 192.0.2.1 extra {\n}\n", 1, ErrorKind::LeaseHeader),
        (
            "\nlease 192.0.2 {\n}\n",
            2,
            ErrorKind::Address("192.0.2".into()),
        ),
        (
            "\nlease 192.0.2.1 {\n  hardware ethernet 02:00:00:00:00:01;\n}\n",
            2,
            ErrorKind::MissingState,
        ),
    ];
    for (text, line, kind) in cases {
        assert_eq!(error(text), (line, kind), "{text}");
    }
}

#[test]
fn missing_state_is_an_error_even_without_hardware() {
    assert_eq!(
        error(&block("  starts never;")),
        (1, ErrorKind::MissingState)
    );
}

#[test]
fn error_message_names_the_line_and_the_problem() {
    let err = parse(&lease_with("  starts soon;"), &Subnets::default()).expect_err("malformed");
    assert_eq!(err.to_string(), "line 4: invalid `starts` value \"soon\"");
}

#[test]
fn bytes_that_are_not_utf8_fail_on_their_line() {
    let err = parse_bytes(b"# ok\n# ok\n\xff\n", &Subnets::default()).expect_err("not UTF-8");
    assert_eq!((err.line(), err.kind()), (3, &ErrorKind::NotUtf8));
}

#[test]
fn utf8_bytes_parse_like_text() {
    let leases = parse_bytes(DHCPD_LEASES.as_bytes(), &fixture_subnets()).expect("parses");
    assert_eq!(leases, fixture_leases());
}

#[test]
fn empty_file_has_no_leases() {
    assert_eq!(parse("", &Subnets::default()).expect("parses"), []);
}

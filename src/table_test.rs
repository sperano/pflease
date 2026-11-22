use std::net::Ipv4Addr;

use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};

use super::{list, render};
use crate::source::FakeSource;
use crate::{Lease, LeaseState, MacAddr, SourceError};

/// 2023-11-14 22:13:20 UTC.
const START_SECONDS: i64 = 1_700_000_000;
/// One hour after [`START_SECONDS`].
const END_SECONDS: i64 = 1_700_003_600;
/// Hours east of UTC of the zone used to check local-time conversion.
const EAST_HOURS: i8 = 2;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(seconds).expect("valid timestamp")
}

fn lease(last_octet: u8) -> Lease {
    Lease {
        ip: Ipv4Addr::new(10, 0, 0, last_octet),
        mac: MacAddr::new([0x02, 0, 0, 0, 0, last_octet]),
        hostname: None,
        start: None,
        end: None,
        state: LeaseState::Free,
        interface: None,
    }
}

fn complete(last_octet: u8, hostname: &str) -> Lease {
    Lease {
        hostname: Some(hostname.into()),
        start: Some(at(START_SECONDS)),
        end: Some(at(END_SECONDS)),
        state: LeaseState::Active,
        interface: Some("lan".into()),
        ..lease(last_octet)
    }
}

fn sample_leases() -> Vec<Lease> {
    vec![complete(10, "printer"), lease(9), complete(100, "nas")]
}

#[test]
fn renders_exact_table_with_missing_values() {
    let expected = "\
IP          MAC                HOSTNAME  START                END                  STATE   INTERFACE
10.0.0.10   02:00:00:00:00:0a  printer   2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
10.0.0.9    02:00:00:00:00:09  -         -                    -                    free    -
10.0.0.100  02:00:00:00:00:64  nas       2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
";
    assert_eq!(render(&sample_leases(), &TimeZone::UTC), expected);
}

#[test]
fn renders_header_only_without_leases() {
    assert_eq!(
        render(&[], &TimeZone::UTC),
        "IP  MAC  HOSTNAME  START  END  STATE  INTERFACE\n"
    );
}

#[test]
fn shows_times_in_the_given_zone() {
    let zone = TimeZone::fixed(Offset::constant(EAST_HOURS));
    let table = render(&[complete(1, "host")], &zone);
    let row = table.lines().nth(1).expect("one lease row");
    assert_eq!(
        row,
        "10.0.0.1  02:00:00:00:00:01  host      2023-11-15 00:13:20  2023-11-15 01:13:20  active  lan"
    );
}

#[test]
fn list_orders_by_ip_numerically() {
    let source = FakeSource::Leases(sample_leases());
    let table = list(&source, &TimeZone::UTC).expect("fake succeeds");
    let ips: Vec<&str> = table
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().next().expect("IP column"))
        .collect();
    assert_eq!(ips, ["10.0.0.9", "10.0.0.10", "10.0.0.100"]);
}

#[test]
fn list_renders_every_column_of_sorted_leases() {
    let source = FakeSource::Leases(vec![complete(2, "b"), complete(1, "a")]);
    let expected = "\
IP        MAC                HOSTNAME  START                END                  STATE   INTERFACE
10.0.0.1  02:00:00:00:00:01  a         2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
10.0.0.2  02:00:00:00:00:02  b         2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
";
    assert_eq!(
        list(&source, &TimeZone::UTC).expect("fake succeeds"),
        expected
    );
}

#[test]
fn list_passes_source_errors_through() {
    let source = FakeSource::Failing("connection refused".into());
    let err = list(&source, &TimeZone::UTC).expect_err("failing source fails");
    assert!(matches!(&err, SourceError::Unavailable(m) if m == "connection refused"));
}

#[test]
fn widths_count_characters_not_bytes() {
    // Ten characters, twenty bytes: wider than the header in both.
    let table = render(
        &[complete(1, "éééééééééé"), complete(2, "abcd")],
        &TimeZone::UTC,
    );
    let expected = "\
IP        MAC                HOSTNAME    START                END                  STATE   INTERFACE
10.0.0.1  02:00:00:00:00:01  éééééééééé  2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
10.0.0.2  02:00:00:00:00:02  abcd        2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
";
    assert_eq!(table, expected);
}

#[test]
fn widths_count_double_width_characters_as_two_columns() {
    // Six characters, twelve terminal columns.
    let table = render(
        &[complete(1, "日本語ホスト"), complete(2, "abcd")],
        &TimeZone::UTC,
    );
    let expected = "\
IP        MAC                HOSTNAME      START                END                  STATE   INTERFACE
10.0.0.1  02:00:00:00:00:01  日本語ホスト  2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
10.0.0.2  02:00:00:00:00:02  abcd          2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
";
    assert_eq!(table, expected);
}

#[test]
fn widens_column_to_longest_cell() {
    let table = render(&[complete(1, "a-long-hostname")], &TimeZone::UTC);
    assert_eq!(
        table.lines().next().expect("header"),
        "IP        MAC                HOSTNAME         START                END                  STATE   INTERFACE"
    );
}

#[test]
fn escapes_control_characters_in_text_columns() {
    let mut hostile = complete(1, "evil\u{1b}[2J");
    hostile.interface = Some("lan\n".into());
    hostile.state = "x\u{7}".parse().expect("unknown state parses");
    let row = render(&[hostile], &TimeZone::UTC)
        .lines()
        .nth(1)
        .expect("one lease row")
        .to_owned();
    assert_eq!(
        row,
        "10.0.0.1  02:00:00:00:00:01  evil\\u{1b}[2J  2023-11-14 22:13:20  2023-11-14 23:13:20  x\\u{7}  lan\\u{a}"
    );
}

#[test]
fn empty_text_shows_as_missing() {
    let mut blank = lease(1);
    blank.hostname = Some(String::new());
    blank.interface = Some(String::new());
    let row = render(&[blank], &TimeZone::UTC)
        .lines()
        .nth(1)
        .expect("one lease row")
        .to_owned();
    assert_eq!(
        row,
        "10.0.0.1  02:00:00:00:00:01  -         -      -    free   -"
    );
}

#[test]
fn escapes_bidi_and_format_characters_in_text_columns() {
    let mut spoof = complete(1, "evil\u{202e}txt.exe");
    spoof.interface = Some("lan\u{2066}\u{200b}".into());
    let row = render(&[spoof], &TimeZone::UTC)
        .lines()
        .nth(1)
        .expect("one lease row")
        .to_owned();
    assert_eq!(
        row,
        "10.0.0.1  02:00:00:00:00:01  evil\\u{202e}txt.exe  2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan\\u{2066}\\u{200b}"
    );
}

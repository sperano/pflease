use std::net::Ipv4Addr;

use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};

use super::{Scope, list, render};
use crate::source::FakeSource;
use crate::{Filter, Lease, LeaseSource, LeaseState, MacAddr, Sort, SortKey, SourceError};

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
    let table = list(
        &source,
        Scope::Leases,
        &Filter::default(),
        Sort::default(),
        &TimeZone::UTC,
    )
    .expect("fake succeeds");
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
        list(
            &source,
            Scope::Leases,
            &Filter::default(),
            Sort::default(),
            &TimeZone::UTC
        )
        .expect("fake succeeds"),
        expected
    );
}

#[test]
fn list_shows_only_leases_the_filter_accepts() {
    let source = FakeSource::Leases(vec![complete(3, "c"), complete(1, "a"), complete(2, "b")]);
    let filter = Filter::default().hostname("A").hostname("c");
    let expected = "\
IP        MAC                HOSTNAME  START                END                  STATE   INTERFACE
10.0.0.1  02:00:00:00:00:01  a         2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
10.0.0.3  02:00:00:00:00:03  c         2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
";
    assert_eq!(
        list(
            &source,
            Scope::Leases,
            &filter,
            Sort::default(),
            &TimeZone::UTC
        )
        .expect("fake succeeds"),
        expected
    );
}

#[test]
fn list_orders_leases_by_the_given_sort() {
    let source = FakeSource::Leases(sample_leases());
    let sort = Sort {
        key: SortKey::Hostname,
        reverse: true,
    };
    let table = list(
        &source,
        Scope::Leases,
        &Filter::default(),
        sort,
        &TimeZone::UTC,
    )
    .expect("fake succeeds");
    let ips: Vec<&str> = table
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().next().expect("IP column"))
        .collect();
    assert_eq!(ips, ["10.0.0.10", "10.0.0.100", "10.0.0.9"]);
}

#[test]
fn list_passes_source_errors_through() {
    let source = FakeSource::Failing("connection refused".into());
    let err = list(
        &source,
        Scope::Leases,
        &Filter::default(),
        Sort::default(),
        &TimeZone::UTC,
    )
    .expect_err("failing source fails");
    assert!(matches!(&err, SourceError::Unavailable(m) if m == "connection refused"));
}

/// A static mapping at `10.0.0.<last_octet>` named `hostname`.
fn mapping(last_octet: u8, hostname: &str) -> Lease {
    Lease {
        hostname: Some(hostname.into()),
        state: LeaseState::Static,
        ..lease(last_octet)
    }
}

#[test]
fn list_all_adds_static_mappings_sorted_with_the_leases() {
    let source = FakeSource::Mapped {
        leases: vec![complete(3, "c"), complete(1, "a")],
        mappings: vec![mapping(2, "fixed")],
    };
    let expected = "\
IP        MAC                HOSTNAME  START                END                  STATE   INTERFACE
10.0.0.1  02:00:00:00:00:01  a         2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
10.0.0.2  02:00:00:00:00:02  fixed     -                    -                    static  -
10.0.0.3  02:00:00:00:00:03  c         2023-11-14 22:13:20  2023-11-14 23:13:20  active  lan
";
    assert_eq!(
        list(
            &source,
            Scope::All,
            &Filter::default(),
            Sort::default(),
            &TimeZone::UTC
        )
        .expect("fake succeeds"),
        expected
    );
}

#[test]
fn list_all_filters_static_mappings_by_state() {
    let source = FakeSource::Mapped {
        leases: vec![complete(1, "a")],
        mappings: vec![mapping(2, "fixed"), mapping(3, "other")],
    };
    let filter = Filter::default().state(LeaseState::Static);
    let table = list(
        &source,
        Scope::All,
        &filter,
        Sort::default(),
        &TimeZone::UTC,
    )
    .expect("fake succeeds");
    let ips: Vec<&str> = table
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().next().expect("IP column"))
        .collect();
    assert_eq!(ips, ["10.0.0.2", "10.0.0.3"]);
}

#[test]
fn list_all_filters_static_mappings_by_state_and_hostname() {
    let source = FakeSource::Mapped {
        leases: vec![complete(1, "fixer")],
        mappings: vec![mapping(2, "fixed"), mapping(3, "other")],
    };
    let filter = Filter::default().state(LeaseState::Static).hostname("fix");
    let table = list(
        &source,
        Scope::All,
        &filter,
        Sort::default(),
        &TimeZone::UTC,
    )
    .expect("fake succeeds");
    let ips: Vec<&str> = table
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().next().expect("IP column"))
        .collect();
    assert_eq!(ips, ["10.0.0.2"]);
}

/// A source whose leases are fine but whose static mappings fail.
struct MappingsFail;

impl LeaseSource for MappingsFail {
    fn leases(&self) -> Result<Vec<Lease>, SourceError> {
        Ok(vec![complete(1, "a")])
    }

    fn static_mappings(&self) -> Result<Vec<Lease>, SourceError> {
        Err(SourceError::Unavailable("no config".into()))
    }
}

#[test]
fn list_without_all_does_not_read_static_mappings() {
    let table = list(
        &MappingsFail,
        Scope::Leases,
        &Filter::default(),
        Sort::default(),
        &TimeZone::UTC,
    )
    .expect("leases only");
    assert_eq!(table.lines().count(), 2, "{table}");
}

#[test]
fn list_all_passes_static_mapping_errors_through() {
    let err = list(
        &MappingsFail,
        Scope::All,
        &Filter::default(),
        Sort::default(),
        &TimeZone::UTC,
    )
    .expect_err("mappings fail");
    assert!(matches!(&err, SourceError::Unavailable(m) if m == "no config"));
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

/// First and last code point of every format-character range, with the
/// escape each must render as.
const FORMAT_BOUNDARIES: [(char, &str); 32] = [
    ('\u{ad}', "\\u{ad}"),
    ('\u{600}', "\\u{600}"),
    ('\u{605}', "\\u{605}"),
    ('\u{61c}', "\\u{61c}"),
    ('\u{6dd}', "\\u{6dd}"),
    ('\u{70f}', "\\u{70f}"),
    ('\u{890}', "\\u{890}"),
    ('\u{891}', "\\u{891}"),
    ('\u{8e2}', "\\u{8e2}"),
    ('\u{180e}', "\\u{180e}"),
    ('\u{200b}', "\\u{200b}"),
    ('\u{200f}', "\\u{200f}"),
    ('\u{202a}', "\\u{202a}"),
    ('\u{202e}', "\\u{202e}"),
    ('\u{2060}', "\\u{2060}"),
    ('\u{2064}', "\\u{2064}"),
    ('\u{2066}', "\\u{2066}"),
    ('\u{206f}', "\\u{206f}"),
    ('\u{feff}', "\\u{feff}"),
    ('\u{fff9}', "\\u{fff9}"),
    ('\u{fffb}', "\\u{fffb}"),
    ('\u{110bd}', "\\u{110bd}"),
    ('\u{110cd}', "\\u{110cd}"),
    ('\u{13430}', "\\u{13430}"),
    ('\u{1343f}', "\\u{1343f}"),
    ('\u{1bca0}', "\\u{1bca0}"),
    ('\u{1bca3}', "\\u{1bca3}"),
    ('\u{1d173}', "\\u{1d173}"),
    ('\u{1d17a}', "\\u{1d17a}"),
    ('\u{e0001}', "\\u{e0001}"),
    ('\u{e0020}', "\\u{e0020}"),
    ('\u{e007f}', "\\u{e007f}"),
];

/// The code points just below the start and just above the end of each
/// format-character range (outside every range and not control
/// characters), which must render unchanged.
const FORMAT_NEIGHBOURS: [char; 41] = [
    '\u{ac}',
    '\u{ae}',
    '\u{5ff}',
    '\u{606}',
    '\u{61b}',
    '\u{61d}',
    '\u{6dc}',
    '\u{6de}',
    '\u{70e}',
    '\u{710}',
    '\u{88f}',
    '\u{892}',
    '\u{8e1}',
    '\u{8e3}',
    '\u{180d}',
    '\u{180f}',
    '\u{200a}',
    '\u{2010}',
    '\u{2029}',
    '\u{202f}',
    '\u{205f}',
    '\u{2065}',
    '\u{2070}',
    '\u{fefe}',
    '\u{ff00}',
    '\u{fff8}',
    '\u{fffc}',
    '\u{110bc}',
    '\u{110be}',
    '\u{110cc}',
    '\u{110ce}',
    '\u{1342f}',
    '\u{13440}',
    '\u{1bc9f}',
    '\u{1bca4}',
    '\u{1d172}',
    '\u{1d17b}',
    '\u{e0000}',
    '\u{e0002}',
    '\u{e001f}',
    '\u{e0080}',
];

/// The lease row with `interface` as the last (unpadded) column.
fn row_with_interface(interface: String) -> String {
    let mut lease = complete(1, "host");
    lease.interface = Some(interface);
    render(&[lease], &TimeZone::UTC)
        .lines()
        .nth(1)
        .expect("one lease row")
        .to_owned()
}

const ROW_PREFIX: &str =
    "10.0.0.1  02:00:00:00:00:01  host      2023-11-14 22:13:20  2023-11-14 23:13:20  active  ";

#[test]
fn escapes_first_and_last_character_of_every_format_range() {
    for (c, escape) in FORMAT_BOUNDARIES {
        assert_eq!(
            row_with_interface(format!("x{c}x")),
            format!("{ROW_PREFIX}x{escape}x"),
            "U+{:04X}",
            u32::from(c)
        );
    }
}

#[test]
fn keeps_printable_neighbours_of_format_ranges() {
    for c in FORMAT_NEIGHBOURS {
        assert_eq!(
            row_with_interface(format!("x{c}x")),
            format!("{ROW_PREFIX}x{c}x"),
            "U+{:04X}",
            u32::from(c)
        );
    }
}

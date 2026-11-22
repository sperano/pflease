use super::{MacAddr, ParseMacError};

const OCTETS: [u8; 6] = [0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0xef];
const CANONICAL: &str = "00:1a:2b:3c:4d:ef";

fn parse(text: &str) -> Result<MacAddr, ParseMacError> {
    text.parse()
}

#[test]
fn parses_lowercase_colons() {
    assert_eq!(parse(CANONICAL), Ok(MacAddr::new(OCTETS)));
}

#[test]
fn parses_dashes() {
    assert_eq!(parse("00-1a-2b-3c-4d-ef"), Ok(MacAddr::new(OCTETS)));
}

#[test]
fn parses_uppercase_and_mixed_case() {
    assert_eq!(parse("00:1A:2B:3C:4D:EF"), Ok(MacAddr::new(OCTETS)));
    assert_eq!(parse("00-1a-2B-3c-4D-eF"), Ok(MacAddr::new(OCTETS)));
}

#[test]
fn displays_lowercase_colons() {
    let mac = parse("00-1A-2B-3C-4D-EF").expect("valid MAC");
    assert_eq!(mac.to_string(), CANONICAL);
}

#[test]
fn display_round_trips() {
    let mac = MacAddr::new([0xff, 0x00, 0x0a, 0xa0, 0x01, 0x10]);
    assert_eq!(mac.to_string(), "ff:00:0a:a0:01:10");
    assert_eq!(parse(&mac.to_string()), Ok(mac));
}

#[test]
fn octets_returns_constructor_input() {
    assert_eq!(MacAddr::new(OCTETS).octets(), OCTETS);
}

#[test]
fn rejects_wrong_length() {
    assert_eq!(parse(""), Err(ParseMacError::Length(0)));
    assert_eq!(parse("00:1a:2b:3c:4d"), Err(ParseMacError::Length(14)));
    assert_eq!(
        parse("00:1a:2b:3c:4d:ef:01"),
        Err(ParseMacError::Length(20))
    );
    assert_eq!(parse("001a2b3c4def"), Err(ParseMacError::Length(12)));
}

#[test]
fn length_counts_characters_not_bytes() {
    assert_eq!(
        parse("00:1a:2b:3c:4d:éf"),
        Err(ParseMacError::Hex("éf".into()))
    );
}

#[test]
fn rejects_mixed_separators() {
    assert_eq!(parse("00:1a-2b:3c:4d:ef"), Err(ParseMacError::Separator));
}

#[test]
fn rejects_other_separators() {
    assert_eq!(parse("00.1a.2b.3c.4d.ef"), Err(ParseMacError::Separator));
    assert_eq!(parse("00:1a:2b:3c:4d.ef"), Err(ParseMacError::Separator));
}

#[test]
fn rejects_misplaced_separators() {
    assert_eq!(parse("001:a2:b3:c4:def"), Err(ParseMacError::Length(16)));
    assert_eq!(parse("0:01a:2b:3c:4d:ef"), Err(ParseMacError::Separator));
}

#[test]
fn rejects_non_hex_digits() {
    assert_eq!(
        parse("00:1a:2g:3c:4d:ef"),
        Err(ParseMacError::Hex("2g".into()))
    );
    assert_eq!(
        parse("zz:1a:2b:3c:4d:ef"),
        Err(ParseMacError::Hex("zz".into()))
    );
}

#[test]
fn rejects_signed_octet() {
    assert_eq!(
        parse("+1:1a:2b:3c:4d:ef"),
        Err(ParseMacError::Hex("+1".into()))
    );
}

#[test]
fn error_messages_name_the_problem() {
    assert_eq!(
        ParseMacError::Length(3).to_string(),
        "MAC address must be 17 characters, got 3"
    );
    assert_eq!(
        ParseMacError::Separator.to_string(),
        "MAC address octets must be separated by all ':' or all '-'"
    );
    assert_eq!(
        ParseMacError::Hex("zz".into()).to_string(),
        "MAC address octet \"zz\" is not two hex digits"
    );
}

use super::{KNOWN, LeaseState, ParseStateError};

fn parse(text: &str) -> Result<LeaseState, ParseStateError> {
    text.parse()
}

#[test]
fn known_states_round_trip() {
    for state in KNOWN {
        let text = state.to_string();
        assert_eq!(parse(&text), Ok(state), "round trip of {text:?}");
    }
}

#[test]
fn parses_isc_names() {
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
    for (text, state) in cases {
        assert_eq!(parse(text), Ok(state), "parsing {text:?}");
    }
}

#[test]
fn parses_kea_names() {
    assert_eq!(parse("default"), Ok(LeaseState::Active));
    assert_eq!(parse("declined"), Ok(LeaseState::Declined));
    assert_eq!(parse("expired-reclaimed"), Ok(LeaseState::ExpiredReclaimed));
}

#[test]
fn kea_default_displays_as_active() {
    assert_eq!(parse("default").expect("known state").to_string(), "active");
}

#[test]
fn displays_canonical_names() {
    assert_eq!(
        LeaseState::ExpiredReclaimed.to_string(),
        "expired-reclaimed"
    );
    assert_eq!(LeaseState::Abandoned.to_string(), "abandoned");
}

#[test]
fn matches_case_insensitively_and_trims() {
    assert_eq!(parse("ACTIVE"), Ok(LeaseState::Active));
    assert_eq!(parse("Default"), Ok(LeaseState::Active));
    assert_eq!(
        parse(" Expired-Reclaimed\n"),
        Ok(LeaseState::ExpiredReclaimed)
    );
}

#[test]
fn keeps_unknown_names_verbatim() {
    let state = parse(" Registered ").expect("unknown names parse");
    let LeaseState::Other(unknown) = &state else {
        panic!("expected Other, got {state:?}");
    };
    assert_eq!(unknown.as_str(), "Registered");
    assert_eq!(state.to_string(), "Registered");
    assert_eq!(parse(&state.to_string()), Ok(state));
}

#[test]
fn rejects_empty() {
    assert_eq!(parse(""), Err(ParseStateError::Empty));
    assert_eq!(parse("  \t"), Err(ParseStateError::Empty));
    assert_eq!(
        ParseStateError::Empty.to_string(),
        "lease state must not be empty"
    );
}

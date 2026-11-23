use std::net::Ipv4Addr;

use super::{ParseStaticMapError, parse, parse_bytes};
use crate::test_support::{CONFIG_XML, fixture_mappings, fixture_subnets};
use crate::{Lease, LeaseState, MacAddr, Subnets};

/// `body` wrapped in `<pfsense><dhcpd>`.
fn config(body: &str) -> String {
    format!("<pfsense><dhcpd>{body}</dhcpd></pfsense>")
}

fn parse_err(text: &str) -> ParseStaticMapError {
    parse(text, &Subnets::default()).expect_err("rejected")
}

#[test]
fn reads_every_mapping_with_a_mac_and_an_address() {
    assert_eq!(
        parse(CONFIG_XML, &fixture_subnets()).expect("parses"),
        fixture_mappings()
    );
}

#[test]
fn mapping_fields_are_trimmed() {
    let text = config(
        "<lan><staticmap><mac> 02:00:00:00:00:01\n</mac>\
         <ipaddr>\t10.0.0.1 </ipaddr><hostname> host </hostname></staticmap></lan>",
    );
    assert_eq!(
        parse(&text, &Subnets::default()).expect("parses"),
        [Lease {
            ip: Ipv4Addr::new(10, 0, 0, 1),
            mac: MacAddr::new([0x02, 0, 0, 0, 0, 0x01]),
            hostname: Some("host".into()),
            start: None,
            end: None,
            state: LeaseState::Static,
            interface: None,
        }]
    );
}

#[test]
fn mapping_without_mac_or_ipaddr_elements_is_skipped() {
    let text = config(
        "<lan><staticmap><ipaddr>10.0.0.1</ipaddr></staticmap>\
         <staticmap><mac>02:00:00:00:00:02</mac></staticmap></lan>",
    );
    assert_eq!(parse(&text, &Subnets::default()).expect("parses"), []);
}

#[test]
fn configuration_without_dhcpd_has_no_mappings() {
    assert_eq!(
        parse("<pfsense><system/></pfsense>", &Subnets::default()).expect("parses"),
        []
    );
}

#[test]
fn rejects_a_root_other_than_pfsense() {
    let err = parse_err("<opnsense><dhcpd/></opnsense>");
    assert_eq!(err, ParseStaticMapError::NotPfSense("opnsense".into()));
    assert_eq!(err.to_string(), "root element is <opnsense>, not <pfsense>");
}

#[test]
fn xml_error_reports_only_its_position() {
    let err = parse_err("<pfsense>\n  <secret>hunter2</oops>\n</pfsense>");
    assert_eq!(
        err,
        ParseStaticMapError::Xml {
            line: 2,
            column: 18
        }
    );
    assert_eq!(err.to_string(), "invalid XML at line 2, column 18");
}

#[test]
fn rejects_a_document_type_declaration() {
    let err = parse_err("<!DOCTYPE pfsense [<!ENTITY x \"y\">]>\n<pfsense/>");
    assert!(
        matches!(err, ParseStaticMapError::Xml { line: 1, .. }),
        "{err:?}"
    );
}

#[test]
fn rejects_an_invalid_mac() {
    let err = parse_err(&config(
        "<opt1><staticmap><mac>02:00:00:00:00</mac><ipaddr>10.0.0.1</ipaddr></staticmap></opt1>",
    ));
    assert_eq!(
        err,
        ParseStaticMapError::InvalidMac {
            interface: "opt1".into(),
            value: "02:00:00:00:00".into(),
        }
    );
    assert_eq!(
        err.to_string(),
        "static mapping on opt1: invalid <mac> \"02:00:00:00:00\""
    );
}

#[test]
fn rejects_an_invalid_ipaddr() {
    let err = parse_err(&config(
        "<lan><staticmap><mac>02:00:00:00:00:01</mac><ipaddr>10.0.0.256</ipaddr></staticmap></lan>",
    ));
    assert_eq!(
        err,
        ParseStaticMapError::InvalidIp {
            interface: "lan".into(),
            value: "10.0.0.256".into(),
        }
    );
    assert_eq!(
        err.to_string(),
        "static mapping on lan: invalid <ipaddr> \"10.0.0.256\""
    );
}

#[test]
fn rejects_bytes_that_are_not_utf8() {
    let err = parse_bytes(b"<pfsense>\xff</pfsense>", &Subnets::default()).expect_err("rejected");
    assert_eq!(err, ParseStaticMapError::NotUtf8);
    assert_eq!(err.to_string(), "not UTF-8 text");
}

#[test]
fn parse_bytes_reads_utf8() {
    assert_eq!(
        parse_bytes(CONFIG_XML.as_bytes(), &fixture_subnets()).expect("parses"),
        fixture_mappings()
    );
}

//! Config tests about the `[interfaces]` table.

use std::net::Ipv4Addr;

use super::tests::{file, load_with, with_config};
use super::{ConfigError, Options};
use crate::Subnets;

fn subnets(pairs: &[(&str, &str)]) -> Subnets {
    Subnets::new(
        pairs
            .iter()
            .map(|(name, range)| ((*name).to_owned(), range.parse().expect("valid range")))
            .collect(),
    )
}

fn load_file(contents: &str, firewall: Option<&str>) -> Result<super::Settings, ConfigError> {
    let (_dir, path) = file(contents);
    let options = Options {
        firewall: firewall.map(str::to_owned),
        ..with_config(&path)
    };
    load_with(&options, &[])
}

#[test]
fn top_level_interfaces_become_subnets_in_name_order() {
    let settings = load_file(
        "host = \"h\"\n[interfaces]\nlan = \"192.0.2.0/24\"\niot = \"198.51.100.10-198.51.100.20\"\n",
        None,
    )
    .expect("loads");
    assert_eq!(
        settings.interfaces,
        subnets(&[
            ("iot", "198.51.100.10-198.51.100.20"),
            ("lan", "192.0.2.0/24")
        ])
    );
    assert_eq!(
        settings
            .interfaces
            .interface_of(Ipv4Addr::new(192, 0, 2, 9)),
        Some("lan")
    );
}

#[test]
fn firewall_interfaces_replace_the_top_level_table() {
    let contents = "host = \"h\"\n[interfaces]\nlan = \"192.0.2.0/24\"\n\
                    [firewalls.lab.interfaces]\nlab = \"10.0.0.0/8\"\n";
    let settings = load_file(contents, Some("lab")).expect("loads");
    assert_eq!(settings.interfaces, subnets(&[("lab", "10.0.0.0/8")]));
}

#[test]
fn empty_firewall_table_keeps_the_top_level_one() {
    let contents = "host = \"h\"\n[interfaces]\nlan = \"192.0.2.0/24\"\n\
                    [firewalls.lab.interfaces]\n";
    let settings = load_file(contents, Some("lab")).expect("loads");
    assert_eq!(settings.interfaces, subnets(&[("lan", "192.0.2.0/24")]));
}

#[test]
fn no_table_means_no_subnets() {
    let settings = load_file("host = \"h\"\n", None).expect("loads");
    assert_eq!(settings.interfaces, Subnets::default());
}

#[test]
fn invalid_subnet_names_its_interface() {
    let err = load_file("host = \"h\"\n[interfaces]\nlan = \"192.0.2.0/33\"\n", None)
        .expect_err("invalid subnet");
    assert!(
        matches!(&err, ConfigError::Interface { name, .. } if name == "lan"),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "invalid subnet for interface `lan`: CIDR prefix length \"33\" is not a number from 0 to 32"
    );
}

use std::net::Ipv4Addr;

use super::Subnets;

fn subnets(pairs: &[(&str, &str)]) -> Subnets {
    Subnets::new(
        pairs
            .iter()
            .map(|(name, range)| ((*name).to_owned(), range.parse().expect("valid range")))
            .collect(),
    )
}

#[test]
fn finds_the_interface_whose_subnet_holds_the_address() {
    let subnets = subnets(&[("lan", "192.0.2.0/24"), ("iot", "198.51.100.0/24")]);
    assert_eq!(
        subnets.interface_of(Ipv4Addr::new(198, 51, 100, 7)),
        Some("iot")
    );
    assert_eq!(
        subnets.interface_of(Ipv4Addr::new(192, 0, 2, 255)),
        Some("lan")
    );
}

#[test]
fn address_outside_every_subnet_has_no_interface() {
    let subnets = subnets(&[("lan", "192.0.2.0/24")]);
    assert_eq!(subnets.interface_of(Ipv4Addr::new(192, 0, 3, 0)), None);
    assert_eq!(
        Subnets::default().interface_of(Ipv4Addr::new(192, 0, 2, 1)),
        None
    );
}

#[test]
fn narrowest_overlapping_subnet_wins_whatever_the_order() {
    let wide_first = subnets(&[("all", "10.0.0.0/8"), ("lab", "10.1.0.0/16")]);
    let narrow_first = subnets(&[("lab", "10.1.0.0/16"), ("all", "10.0.0.0/8")]);
    let ip = Ipv4Addr::new(10, 1, 2, 3);
    assert_eq!(wide_first.interface_of(ip), Some("lab"));
    assert_eq!(narrow_first.interface_of(ip), Some("lab"));
    assert_eq!(
        wide_first.interface_of(Ipv4Addr::new(10, 2, 0, 1)),
        Some("all")
    );
}

#[test]
fn first_of_equally_narrow_subnets_wins() {
    let subnets = subnets(&[("b", "192.0.2.0/24"), ("a", "192.0.2.0/24")]);
    assert_eq!(subnets.interface_of(Ipv4Addr::new(192, 0, 2, 1)), Some("b"));
}

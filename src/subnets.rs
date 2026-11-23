//! Which firewall interface serves an address.
//!
//! ISC's `dhcpd.leases` records no interface, so the source derives it from
//! the subnets configured for each interface (`[interfaces]` in the config
//! file).

use std::net::Ipv4Addr;

use crate::IpRange;

/// Interface names and the address ranges they serve.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Subnets(Vec<(String, IpRange)>);

impl Subnets {
    /// Subnets from `(interface, range)` pairs.
    #[must_use]
    pub fn new(subnets: Vec<(String, IpRange)>) -> Self {
        Self(subnets)
    }

    /// The interface whose range holds `ip`: the narrowest one when ranges
    /// overlap, the first given among equally narrow ones; `None` when no
    /// range holds it.
    #[must_use]
    pub fn interface_of(&self, ip: Ipv4Addr) -> Option<&str> {
        self.0
            .iter()
            .filter(|(_, range)| range.contains(ip))
            .min_by_key(|(_, range)| width(*range))
            .map(|(name, _)| name.as_str())
    }
}

/// How many addresses past the first `range` covers.
fn width(range: IpRange) -> u32 {
    range.last().to_bits() - range.first().to_bits()
}

#[cfg(test)]
#[path = "subnets_test.rs"]
mod tests;

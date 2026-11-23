//! Inclusive IPv4 address ranges, for `--ip`.

use std::fmt;
use std::net::{AddrParseError, Ipv4Addr};
use std::str::FromStr;

/// Bits in an IPv4 address, the largest CIDR prefix length.
const ADDRESS_BITS: u8 = 32;
/// Separator between the address and the prefix length of a CIDR block.
const CIDR_SEPARATOR: char = '/';
/// Separator between the two ends of an explicit range.
const RANGE_SEPARATOR: char = '-';

/// An inclusive range of IPv4 addresses.
///
/// Parses a CIDR block (`10.0.0.0/24`), an explicit range whose ends are
/// both included (`10.0.0.10-10.0.0.50`) or a single address. A CIDR block
/// whose address has host bits set covers its whole network, so
/// `10.0.0.7/24` is `10.0.0.0/24`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IpRange {
    first: Ipv4Addr,
    last: Ipv4Addr,
}

impl IpRange {
    /// Whether `ip` lies in the range, both ends included.
    #[must_use]
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        (self.first..=self.last).contains(&ip)
    }

    /// The lowest address of the range.
    #[must_use]
    pub const fn first(&self) -> Ipv4Addr {
        self.first
    }

    /// The highest address of the range.
    #[must_use]
    pub const fn last(&self) -> Ipv4Addr {
        self.last
    }
}

/// Why a string is not an address range.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseIpRangeError {
    /// An address is not dotted-quad IPv4; holds the address text.
    #[error("{0:?} is not an IPv4 address")]
    Address(String),
    /// The CIDR prefix length is not a number from 0 to 32; holds its text.
    #[error("CIDR prefix length {0:?} is not a number from 0 to {ADDRESS_BITS}")]
    PrefixLength(String),
    /// The range ends after it starts.
    #[error("range {first}-{last} is reversed: {first} is after {last}")]
    Reversed {
        /// The start the input gave.
        first: Ipv4Addr,
        /// The end the input gave.
        last: Ipv4Addr,
    },
}

impl FromStr for IpRange {
    type Err = ParseIpRangeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((address, length)) = s.split_once(CIDR_SEPARATOR) {
            return Ok(cidr(parse_address(address)?, parse_length(length)?));
        }
        if let Some((first, last)) = s.split_once(RANGE_SEPARATOR) {
            return range(parse_address(first)?, parse_address(last)?);
        }
        let ip = parse_address(s)?;
        Ok(Self {
            first: ip,
            last: ip,
        })
    }
}

/// The block of `length` leading bits shared with `address`.
fn cidr(address: Ipv4Addr, length: u8) -> IpRange {
    let host_bits = u32::from(ADDRESS_BITS - length);
    let mask = u32::MAX.checked_shl(host_bits).unwrap_or(0);
    let network = address.to_bits() & mask;
    IpRange {
        first: Ipv4Addr::from_bits(network),
        last: Ipv4Addr::from_bits(network | !mask),
    }
}

/// The range from `first` to `last`, refused when reversed.
fn range(first: Ipv4Addr, last: Ipv4Addr) -> Result<IpRange, ParseIpRangeError> {
    if first > last {
        return Err(ParseIpRangeError::Reversed { first, last });
    }
    Ok(IpRange { first, last })
}

fn parse_address(text: &str) -> Result<Ipv4Addr, ParseIpRangeError> {
    text.parse()
        .map_err(|_: AddrParseError| ParseIpRangeError::Address(text.to_owned()))
}

/// Parses a prefix length; `u8::from_str` alone would accept a `+` sign.
fn parse_length(text: &str) -> Result<u8, ParseIpRangeError> {
    let invalid = || ParseIpRangeError::PrefixLength(text.to_owned());
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    match text.parse::<u8>() {
        Ok(length) if length <= ADDRESS_BITS => Ok(length),
        _ => Err(invalid()),
    }
}

impl fmt::Display for IpRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.first == self.last {
            write!(f, "{}", self.first)
        } else {
            write!(f, "{}{RANGE_SEPARATOR}{}", self.first, self.last)
        }
    }
}

#[cfg(test)]
#[path = "ip_range_test.rs"]
mod tests;

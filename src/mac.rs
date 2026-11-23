//! Ethernet hardware addresses.

use std::fmt;
use std::str::FromStr;

/// Number of octets in an Ethernet address.
pub(crate) const OCTETS: usize = 6;
/// Hex digits per octet in the textual form.
pub(crate) const DIGITS_PER_OCTET: usize = 2;
/// Length of the textual form: six two-digit octets and five separators.
const TEXT_LEN: usize = OCTETS * DIGITS_PER_OCTET + OCTETS - 1;
/// Radix of each octet in the textual form.
const HEX_RADIX: u32 = 16;

/// A 48-bit Ethernet hardware address.
///
/// Parses `aa:bb:cc:dd:ee:ff` and `aa-bb-cc-dd-ee-ff` in any letter case
/// (one separator throughout) and always displays as lowercase hex with
/// colons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacAddr([u8; OCTETS]);

impl MacAddr {
    /// Builds an address from its octets.
    #[must_use]
    pub const fn new(octets: [u8; OCTETS]) -> Self {
        Self(octets)
    }

    /// The address's octets.
    #[must_use]
    pub const fn octets(self) -> [u8; OCTETS] {
        self.0
    }
}

/// Why a string is not a hardware address.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseMacError {
    /// The input is not exactly 17 characters long; holds its character
    /// count.
    #[error("MAC address must be {TEXT_LEN} characters, got {0}")]
    Length(usize),
    /// A separator is neither `:` nor `-`, or the two are mixed.
    #[error("MAC address octets must be separated by all ':' or all '-'")]
    Separator,
    /// An octet is not two hex digits.
    #[error("MAC address octet {0:?} is not two hex digits")]
    Hex(String),
}

impl FromStr for MacAddr {
    type Err = ParseMacError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let len = s.chars().count();
        if len != TEXT_LEN {
            return Err(ParseMacError::Length(len));
        }
        let separator = separator_of(s)?;
        let mut octets = [0; OCTETS];
        for (octet, text) in octets.iter_mut().zip(s.split(separator)) {
            *octet = parse_octet(text)?;
        }
        Ok(Self(octets))
    }
}

/// The separator `s` uses, checked to be the same at every position.
fn separator_of(s: &str) -> Result<char, ParseMacError> {
    let mut separators = s
        .chars()
        .skip(DIGITS_PER_OCTET)
        .step_by(DIGITS_PER_OCTET + 1);
    let first = separators.next().ok_or(ParseMacError::Separator)?;
    if first != ':' && first != '-' {
        return Err(ParseMacError::Separator);
    }
    if separators.all(|c| c == first) {
        Ok(first)
    } else {
        Err(ParseMacError::Separator)
    }
}

/// Parses one two-digit hex octet; `from_str_radix` alone would accept a sign.
pub(crate) fn parse_octet(text: &str) -> Result<u8, ParseMacError> {
    let invalid = || ParseMacError::Hex(text.to_owned());
    if text.len() != DIGITS_PER_OCTET || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    u8::from_str_radix(text, HEX_RADIX).map_err(|_| invalid())
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, octet) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(":")?;
            }
            write!(f, "{octet:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "mac_test.rs"]
mod tests;

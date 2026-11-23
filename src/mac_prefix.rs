//! Leading octets of a hardware address, for `--mac`.

use std::fmt;
use std::str::FromStr;

use crate::MacAddr;
use crate::mac::{DIGITS_PER_OCTET, OCTETS, parse_octet};

/// Characters accepted between hex digit groups, in any mix.
const SEPARATORS: [char; 3] = [':', '-', '.'];

/// The first one to six octets of a hardware address: a full MAC, an OUI
/// (three octets) or any other prefix.
///
/// Parses hex digit groups of even length separated by `:`, `-` or `.` in
/// any mix and letter case, so `AA:bb-CC`, `aabb.cc` and `aabbcc` are the
/// same prefix. A full six-octet prefix matches only that address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacPrefix(Vec<u8>);

impl MacPrefix {
    /// Whether `mac` starts with this prefix.
    #[must_use]
    pub fn matches(&self, mac: MacAddr) -> bool {
        mac.octets().starts_with(&self.0)
    }
}

/// Why a string is not a hardware address prefix.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseMacPrefixError {
    /// The input holds no hex digits.
    #[error("MAC prefix must not be empty")]
    Empty,
    /// A group between separators is empty, has an odd number of digits or
    /// a non-hex character; holds the group.
    #[error("MAC prefix group {0:?} is not an even number of hex digits")]
    Group(String),
    /// The input holds more than six octets; holds their count.
    #[error("MAC prefix has {0} octets, at most {OCTETS} allowed")]
    TooLong(usize),
}

impl FromStr for MacPrefix {
    type Err = ParseMacPrefixError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseMacPrefixError::Empty);
        }
        let mut octets = Vec::with_capacity(OCTETS);
        for group in s.split(SEPARATORS) {
            parse_group(group, &mut octets)?;
        }
        if octets.len() > OCTETS {
            return Err(ParseMacPrefixError::TooLong(octets.len()));
        }
        Ok(Self(octets))
    }
}

/// Appends the octets of one separator-free `group` to `octets`.
fn parse_group(group: &str, octets: &mut Vec<u8>) -> Result<(), ParseMacPrefixError> {
    let invalid = || ParseMacPrefixError::Group(group.to_owned());
    if group.is_empty() || !group.is_ascii() || group.len() % DIGITS_PER_OCTET != 0 {
        return Err(invalid());
    }
    for start in (0..group.len()).step_by(DIGITS_PER_OCTET) {
        let pair = &group[start..start + DIGITS_PER_OCTET];
        octets.push(parse_octet(pair).map_err(|_| invalid())?);
    }
    Ok(())
}

impl fmt::Display for MacPrefix {
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
#[path = "mac_prefix_test.rs"]
mod tests;

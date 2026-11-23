//! The order `pflease list` shows leases in.
//!
//! A [`Sort`] compares leases by one [`SortKey`]: addresses numerically,
//! hardware addresses byte by byte, times chronologically, and hostnames,
//! states and interfaces by their text ignoring case. Leases missing the
//! key's value (no hostname, start, end or interface, or an empty hostname
//! or interface, which the table shows as missing too) sort last whether or
//! not the order is reversed. Leases with equal values follow in ascending
//! IP order, also when reversed, and the sort is stable, so the output is
//! deterministic.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use crate::Lease;

/// The column leases are sorted by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SortKey {
    /// Leased address, numerically.
    #[default]
    Ip,
    /// Hardware address.
    Mac,
    /// Hostname, ignoring case.
    Hostname,
    /// Start time.
    Start,
    /// End time.
    End,
    /// Binding state name, ignoring case.
    State,
    /// Interface name, ignoring case.
    Interface,
}

/// Every key, in column order.
const KEYS: [SortKey; 7] = [
    SortKey::Ip,
    SortKey::Mac,
    SortKey::Hostname,
    SortKey::Start,
    SortKey::End,
    SortKey::State,
    SortKey::Interface,
];

/// Why a string is not a sort key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseSortKeyError {
    /// The name is not one of [`SortKey::names`].
    #[error("unknown sort key {0:?}")]
    Unknown(String),
}

impl SortKey {
    /// The key's name, as `--sort` takes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ip => "ip",
            Self::Mac => "mac",
            Self::Hostname => "hostname",
            Self::Start => "start",
            Self::End => "end",
            Self::State => "state",
            Self::Interface => "interface",
        }
    }

    /// The names of every key, in column order.
    pub fn names() -> impl Iterator<Item = &'static str> {
        KEYS.into_iter().map(Self::as_str)
    }
}

impl FromStr for SortKey {
    type Err = ParseSortKeyError;

    /// Parses a key name, ignoring case and surrounding whitespace.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let name = s.trim();
        KEYS.into_iter()
            .find(|key| name.eq_ignore_ascii_case(key.as_str()))
            .ok_or_else(|| ParseSortKeyError::Unknown(s.to_owned()))
    }
}

impl fmt::Display for SortKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An order of leases: a key and a direction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sort {
    /// What to compare.
    pub key: SortKey,
    /// Whether to put the greatest values first.
    pub reverse: bool,
}

impl Sort {
    /// Sorts `leases` in place in this order.
    pub fn apply(self, leases: &mut [Lease]) {
        leases.sort_by(|a, b| self.compare(a, b));
    }

    /// How `a` compares with `b` in this order: by key, missing values
    /// last, then by ascending IP.
    #[must_use]
    pub fn compare(self, a: &Lease, b: &Lease) -> Ordering {
        self.compare_key(a, b).then_with(|| a.ip.cmp(&b.ip))
    }

    fn compare_key(self, a: &Lease, b: &Lease) -> Ordering {
        match self.key {
            SortKey::Ip => self.direct(a.ip.cmp(&b.ip)),
            SortKey::Mac => self.direct(a.mac.cmp(&b.mac)),
            SortKey::Hostname => self.text(a.hostname.as_deref(), b.hostname.as_deref()),
            SortKey::Start => self.present_first(a.start.as_ref(), b.start.as_ref(), Ord::cmp),
            SortKey::End => self.present_first(a.end.as_ref(), b.end.as_ref(), Ord::cmp),
            SortKey::State => self.direct(cmp_ignoring_case(a.state.as_str(), b.state.as_str())),
            SortKey::Interface => self.text(a.interface.as_deref(), b.interface.as_deref()),
        }
    }

    /// Compares optional text ignoring case; empty counts as missing.
    fn text(self, a: Option<&str>, b: Option<&str>) -> Ordering {
        self.present_first(non_empty(a), non_empty(b), |a, b| cmp_ignoring_case(a, b))
    }

    /// Orders present values by `cmp` in this direction, then missing ones.
    fn present_first<T>(
        self,
        a: Option<T>,
        b: Option<T>,
        cmp: impl FnOnce(&T, &T) -> Ordering,
    ) -> Ordering {
        match (a, b) {
            (Some(a), Some(b)) => self.direct(cmp(&a, &b)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }

    /// `ordering` in this direction.
    fn direct(self, ordering: Ordering) -> Ordering {
        if self.reverse {
            ordering.reverse()
        } else {
            ordering
        }
    }
}

/// `text` unless it is missing or empty.
fn non_empty(text: Option<&str>) -> Option<&str> {
    text.filter(|text| !text.is_empty())
}

/// Compares `a` and `b` by their lowercased characters, without allocating.
fn cmp_ignoring_case(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

#[cfg(test)]
#[path = "sort_test.rs"]
mod tests;

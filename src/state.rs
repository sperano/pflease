//! Lease binding states of the ISC and Kea DHCP servers.
//!
//! pfSense runs ISC `dhcpd` or, on newer releases, Kea. Both name their
//! states in text (ISC's `binding state` in `dhcpd.leases`, Kea's documented
//! names for its numeric `state` column), and [`LeaseState`] parses either:
//!
//! | Input                                   | Variant                         |
//! |-----------------------------------------|---------------------------------|
//! | ISC `active`, Kea `default`             | [`LeaseState::Active`]          |
//! | ISC `free`                              | [`LeaseState::Free`]            |
//! | ISC `expired`                           | [`LeaseState::Expired`]         |
//! | ISC `released`                          | [`LeaseState::Released`]        |
//! | ISC `abandoned`                         | [`LeaseState::Abandoned`]       |
//! | ISC `reset`                             | [`LeaseState::Reset`]           |
//! | ISC `backup`                            | [`LeaseState::Backup`]          |
//! | ISC `bootp`                             | [`LeaseState::Bootp`]           |
//! | ISC `reserved`                          | [`LeaseState::Reserved`]        |
//! | Kea `declined`                          | [`LeaseState::Declined`]        |
//! | Kea `expired-reclaimed`                 | [`LeaseState::ExpiredReclaimed`]|
//! | pfSense static mapping, `static`        | [`LeaseState::Static`]          |
//! | anything else                           | [`LeaseState::Other`]           |
//!
//! Kea's `default` is its state for an assigned, in-use lease, which is what
//! ISC calls `active`, so the two share a variant and display as `active`.
//! Kea's `expired-reclaimed` stays apart from ISC's `expired`: Kea has
//! already freed the address, ISC has not. Kea's `declined` stays apart from
//! ISC's `abandoned` although both mean a client reported an address
//! conflict, because their recovery differs. Neither server records a
//! client with a static mapping as a lease; [`LeaseState::Static`] marks the
//! mappings pflease reads from pfSense's configuration. Surrounding whitespace is
//! trimmed and names match case-insensitively; an unknown name is kept in
//! [`LeaseState::Other`], trimmed but in its original case, rather than
//! guessed into a known state.

use std::fmt;
use std::str::FromStr;

/// The binding state of a lease.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LeaseState {
    /// Assigned and in use (ISC `active`, Kea `default`).
    Active,
    /// Available for assignment (ISC).
    Free,
    /// Ended, not yet reclaimed (ISC).
    Expired,
    /// Given back by the client (ISC).
    Released,
    /// Withheld after an address conflict (ISC).
    Abandoned,
    /// Freed by an administrator through OMAPI (ISC).
    Reset,
    /// Held for the failover peer (ISC).
    Backup,
    /// Assigned to a BOOTP client (ISC).
    Bootp,
    /// Reserved for a host (ISC).
    Reserved,
    /// Declined by the client after an address conflict (Kea).
    Declined,
    /// Expired and already reclaimed by the server (Kea).
    ExpiredReclaimed,
    /// A fixed address from a pfSense static mapping, not a lease.
    Static,
    /// A state name neither backend is known to use.
    Other(UnknownState),
}

/// Every state but [`LeaseState::Other`], the names [`LeaseState::from_str`]
/// recognises.
const KNOWN: [LeaseState; 12] = [
    LeaseState::Active,
    LeaseState::Free,
    LeaseState::Expired,
    LeaseState::Released,
    LeaseState::Abandoned,
    LeaseState::Reset,
    LeaseState::Backup,
    LeaseState::Bootp,
    LeaseState::Reserved,
    LeaseState::Declined,
    LeaseState::ExpiredReclaimed,
    LeaseState::Static,
];

/// Kea's name for an assigned lease, parsed as [`LeaseState::Active`].
const KEA_DEFAULT: &str = "default";

/// A state name that is not one of the known ones, as it was read minus
/// surrounding whitespace.
///
/// Only [`LeaseState::from_str`] builds one, so a known name can never hide
/// in [`LeaseState::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnknownState(String);

impl UnknownState {
    /// The name as it was read, trimmed, in its original case.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a string is not a lease state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseStateError {
    /// The input is empty or only whitespace.
    #[error("lease state must not be empty")]
    Empty,
}

impl LeaseState {
    /// The name [`fmt::Display`] prints: the canonical name of a known state,
    /// the trimmed name as read for [`LeaseState::Other`].
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Other(unknown) => unknown.as_str(),
            known => known.canonical_name().unwrap_or_default(),
        }
    }

    /// The canonical names of the known states, in declaration order: every
    /// name [`LeaseState::from_str`] maps to a variant other than
    /// [`LeaseState::Other`], except Kea's `default` alias of `active`.
    pub fn known_names() -> impl Iterator<Item = &'static str> {
        KNOWN.into_iter().filter_map(|state| state.canonical_name())
    }

    /// The canonical name of a known state; `None` for [`LeaseState::Other`].
    const fn canonical_name(&self) -> Option<&'static str> {
        Some(match self {
            Self::Active => "active",
            Self::Free => "free",
            Self::Expired => "expired",
            Self::Released => "released",
            Self::Abandoned => "abandoned",
            Self::Reset => "reset",
            Self::Backup => "backup",
            Self::Bootp => "bootp",
            Self::Reserved => "reserved",
            Self::Declined => "declined",
            Self::ExpiredReclaimed => "expired-reclaimed",
            Self::Static => "static",
            Self::Other(_) => return None,
        })
    }
}

impl FromStr for LeaseState {
    type Err = ParseStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseStateError::Empty);
        }
        if s.eq_ignore_ascii_case(KEA_DEFAULT) {
            return Ok(Self::Active);
        }
        let known = KNOWN
            .into_iter()
            .find(|state| s.eq_ignore_ascii_case(state.as_str()));
        Ok(known.unwrap_or_else(|| Self::Other(UnknownState(s.to_owned()))))
    }
}

impl fmt::Display for LeaseState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "state_test.rs"]
mod tests;

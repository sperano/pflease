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
//! | anything else                           | [`LeaseState::Other`]           |
//!
//! Kea's `default` is its state for an assigned, in-use lease, which is what
//! ISC calls `active`, so the two share a variant and display as `active`.
//! Kea's `expired-reclaimed` stays apart from ISC's `expired`: Kea has
//! already freed the address, ISC has not. Kea's `declined` stays apart from
//! ISC's `abandoned` although both mean a client reported an address
//! conflict, because their recovery differs. Names match case-insensitively;
//! an unknown name is kept verbatim in [`LeaseState::Other`] rather than
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
    /// A state name neither backend is known to use.
    Other(UnknownState),
}

/// Every state but [`LeaseState::Other`], the names [`LeaseState::from_str`]
/// recognises.
const KNOWN: [LeaseState; 11] = [
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
];

/// Kea's name for an assigned lease, parsed as [`LeaseState::Active`].
const KEA_DEFAULT: &str = "default";

/// A state name that is not one of the known ones, as it was read.
///
/// Only [`LeaseState::from_str`] builds one, so a known name can never hide
/// in [`LeaseState::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnknownState(String);

impl UnknownState {
    /// The name as it was read.
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
    /// the name as read for [`LeaseState::Other`].
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
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
            Self::Other(unknown) => unknown.as_str(),
        }
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

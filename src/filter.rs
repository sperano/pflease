//! Which leases `pflease list` shows.
//!
//! A [`Filter`] holds one list of accepted values per criterion. A lease
//! passes when it satisfies every criterion that has values (AND), and it
//! satisfies a criterion when it matches any one of its values (OR). A
//! criterion without values accepts every lease, so the default filter
//! accepts everything.

use crate::{IpRange, Lease, LeaseState, MacPrefix};

/// Criteria a lease must meet to be shown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Lowercased hostname substrings.
    hostnames: Vec<String>,
    /// Hardware address prefixes.
    macs: Vec<MacPrefix>,
    /// Address ranges.
    ips: Vec<IpRange>,
    /// Binding states.
    states: Vec<LeaseState>,
    /// Lowercased interface names.
    interfaces: Vec<String>,
}

impl Filter {
    /// Also accepts leases whose hostname contains `pattern`, ignoring case.
    /// A lease without a hostname, or with an empty one, never matches.
    #[must_use]
    pub fn hostname(mut self, pattern: &str) -> Self {
        self.hostnames.push(pattern.to_lowercase());
        self
    }

    /// Also accepts leases whose hardware address starts with `prefix`.
    #[must_use]
    pub fn mac(mut self, prefix: MacPrefix) -> Self {
        self.macs.push(prefix);
        self
    }

    /// Also accepts leases whose address lies in `range`.
    #[must_use]
    pub fn ip(mut self, range: IpRange) -> Self {
        self.ips.push(range);
        self
    }

    /// Also accepts leases in `state`.
    #[must_use]
    pub fn state(mut self, state: LeaseState) -> Self {
        self.states.push(state);
        self
    }

    /// Also accepts leases served on the interface `name`, ignoring case.
    /// A lease with no known interface, or an empty one, never matches.
    #[must_use]
    pub fn interface(mut self, name: &str) -> Self {
        self.interfaces.push(name.to_lowercase());
        self
    }

    /// Whether `lease` meets every criterion.
    #[must_use]
    pub fn matches(&self, lease: &Lease) -> bool {
        self.matches_hostname(lease)
            && any_or_unset(&self.macs, |prefix| prefix.matches(lease.mac))
            && any_or_unset(&self.ips, |range| range.contains(lease.ip))
            && any_or_unset(&self.states, |state| *state == lease.state)
            && self.matches_interface(lease)
    }

    fn matches_hostname(&self, lease: &Lease) -> bool {
        if self.hostnames.is_empty() {
            return true;
        }
        let Some(hostname) = lowercase_text(lease.hostname.as_deref()) else {
            return false;
        };
        self.hostnames
            .iter()
            .any(|pattern| hostname.contains(pattern.as_str()))
    }

    fn matches_interface(&self, lease: &Lease) -> bool {
        if self.interfaces.is_empty() {
            return true;
        }
        let Some(interface) = lowercase_text(lease.interface.as_deref()) else {
            return false;
        };
        self.interfaces.contains(&interface)
    }
}

/// `text` lowercased, or `None` when it is missing or empty: the table shows
/// both as `-`, so neither can match a pattern.
fn lowercase_text(text: Option<&str>) -> Option<String> {
    text.filter(|text| !text.is_empty()).map(str::to_lowercase)
}

/// Whether `values` is empty or one of them satisfies `accepts`.
fn any_or_unset<T>(values: &[T], accepts: impl Fn(&T) -> bool) -> bool {
    values.is_empty() || values.iter().any(accepts)
}

#[cfg(test)]
#[path = "filter_test.rs"]
mod tests;

//! One DHCP lease, independent of the backend that reported it.

use std::net::Ipv4Addr;

use jiff::Timestamp;

use crate::{LeaseState, MacAddr};

/// A DHCP lease as reported by the firewall.
///
/// Fields a backend does not always provide are optional: ISC omits the
/// hostname for clients that send none and records no interface name, and a
/// `free` lease may carry no end time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    /// Leased address.
    pub ip: Ipv4Addr,
    /// Hardware address of the client.
    pub mac: MacAddr,
    /// Hostname the client sent, if any.
    pub hostname: Option<String>,
    /// When the lease started.
    pub start: Option<Timestamp>,
    /// When the lease ends or ended.
    pub end: Option<Timestamp>,
    /// Binding state.
    pub state: LeaseState,
    /// Firewall interface serving the lease's subnet, if known.
    pub interface: Option<String>,
}

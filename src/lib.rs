//! Library side of `pflease`: the lease model, the sources that fetch it and
//! the filter and sort that select what to show and in which order.
//!
//! The binary only parses the command line and prints; everything it shows
//! comes through [`LeaseSource`], so display and filter code never depends on
//! how the leases were fetched.

pub mod config;
pub mod filter;
pub mod ip_range;
pub mod lease;
pub mod mac;
pub mod mac_prefix;
pub mod sort;
pub mod source;
pub mod state;
pub mod table;

pub use filter::Filter;
pub use ip_range::{IpRange, ParseIpRangeError};
pub use lease::Lease;
pub use mac::{MacAddr, ParseMacError};
pub use mac_prefix::{MacPrefix, ParseMacPrefixError};
pub use sort::{ParseSortKeyError, Sort, SortKey};
pub use source::{LeaseSource, SourceError};
pub use state::{LeaseState, ParseStateError, UnknownState};

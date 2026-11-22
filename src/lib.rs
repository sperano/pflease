//! Library side of `pflease`: the lease model and the sources that fetch it.
//!
//! The binary only parses the command line and prints; everything it shows
//! comes through [`LeaseSource`], so display and filter code never depends on
//! how the leases were fetched.

pub mod lease;
pub mod mac;
pub mod source;
pub mod state;

pub use lease::Lease;
pub use mac::{MacAddr, ParseMacError};
pub use source::{LeaseSource, SourceError};
pub use state::{LeaseState, ParseStateError, UnknownState};

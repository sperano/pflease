//! Where leases come from.
//!
//! The trait is synchronous: the CLI makes one fetch per run and has nothing
//! to overlap it with, so an async runtime would only add weight. Revisit
//! when a source needs concurrent requests.

use crate::Lease;

/// Why a source could not produce its leases.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SourceError {
    /// The source could not be reached or read.
    #[error("lease source unavailable: {0}")]
    Unavailable(String),
}

/// A backend that reports the firewall's DHCP leases.
pub trait LeaseSource {
    /// Every lease the backend currently knows, in the backend's order.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] when the backend cannot be reached or its
    /// answer cannot be read.
    fn leases(&self) -> Result<Vec<Lease>, SourceError>;
}

/// A source for tests: returns fixed leases, or fails with a fixed message.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) enum FakeSource {
    /// Returns these leases on every call.
    Leases(Vec<Lease>),
    /// Fails with [`SourceError::Unavailable`] carrying this message.
    Failing(String),
}

#[cfg(test)]
impl LeaseSource for FakeSource {
    fn leases(&self) -> Result<Vec<Lease>, SourceError> {
        match self {
            Self::Leases(leases) => Ok(leases.clone()),
            Self::Failing(message) => Err(SourceError::Unavailable(message.clone())),
        }
    }
}

#[cfg(test)]
#[path = "source_test.rs"]
mod tests;

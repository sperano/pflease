//! Where leases come from.
//!
//! The trait is synchronous: the CLI makes one fetch per run and has nothing
//! to overlap it with, so an async runtime would only add weight. Revisit
//! when a source needs concurrent requests.

use crate::config::Settings;
use crate::isc::{self, ParseLeasesError};
use crate::ssh::{Ssh, SshError};
use crate::{Lease, Subnets};

/// Why a source could not produce its leases.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SourceError {
    /// The source could not be reached or read.
    #[error("lease source unavailable: {0}")]
    Unavailable(String),
    /// The lease file could not be read over SSH.
    #[error(transparent)]
    Ssh(#[from] SshError),
    /// The lease file was read but is not a lease database pflease can read.
    #[error("cannot parse lease file {path}: {source}")]
    Parse {
        path: String,
        source: ParseLeasesError,
    },
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

/// ISC `dhcpd`'s lease database, read over SSH; see [`isc`] for how it is
/// parsed.
#[derive(Debug, Clone)]
pub struct IscSshSource {
    ssh: Ssh,
    lease_file: String,
    subnets: Subnets,
}

impl IscSshSource {
    /// Reads `settings.lease_file` on `settings.host`, naming interfaces
    /// from `settings.interfaces`.
    #[must_use]
    pub fn new(settings: &Settings) -> Self {
        Self::with_ssh(
            Ssh::new(settings),
            settings.lease_file.clone(),
            settings.interfaces.clone(),
        )
    }

    /// Reads `lease_file` through `ssh`, naming interfaces from `subnets`.
    #[must_use]
    pub const fn with_ssh(ssh: Ssh, lease_file: String, subnets: Subnets) -> Self {
        Self {
            ssh,
            lease_file,
            subnets,
        }
    }
}

impl LeaseSource for IscSshSource {
    fn leases(&self) -> Result<Vec<Lease>, SourceError> {
        let bytes = self.ssh.read_file(&self.lease_file)?;
        isc::parse_bytes(&bytes, &self.subnets).map_err(|source| SourceError::Parse {
            path: self.lease_file.clone(),
            source,
        })
    }
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

#[cfg(test)]
#[path = "isc_source_test.rs"]
mod isc_tests;

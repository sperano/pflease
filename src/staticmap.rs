//! Parser for the DHCP static mappings in pfSense's `/conf/config.xml`.
//!
//! ISC `dhcpd` hands a client with a static mapping its fixed address
//! without recording a lease, so `dhcpd.leases` never shows it; pfSense's
//! own lease page reads the mappings from its configuration instead, and so
//! does `pflease list --all`. Each mapping sits in
//! `<pfsense><dhcpd><interface><staticmap>` with `<mac>`, `<ipaddr>` and
//! `<hostname>` children; Kea on pfSense keeps them in the same place.
//!
//! The file also holds secrets (password hashes, keys, certificates), so
//! nothing but the three fields above leaves this module: an XML error
//! reports only its position, never the text around it. Decisions per case:
//!
//! - A mapping with no or an empty `<ipaddr>` is skipped: pfSense then gives
//!   the client a pool address, which `dhcpd.leases` already records.
//! - A mapping with no or an empty `<mac>` (one matched by client
//!   identifier only) is skipped: a [`Lease`] always has a MAC.
//! - A `<mac>` or `<ipaddr>` that does not parse is an error naming the
//!   interface element and the value.
//! - A root element other than `<pfsense>` is an error: the file is not a
//!   pfSense configuration. A configuration with no `<dhcpd>` has no
//!   mappings.
//! - An empty `<hostname>` is none. The interface comes from [`Subnets`],
//!   as for leases; an address outside every subnet has none.

use std::net::Ipv4Addr;

use roxmltree::{Document, Node};

use crate::{Lease, LeaseState, MacAddr, Subnets};

/// Root element of a pfSense configuration.
const ROOT: &str = "pfsense";
/// Element holding the IPv4 DHCP server settings, one child per interface.
const DHCPD: &str = "dhcpd";
/// Element of one static mapping.
const STATICMAP: &str = "staticmap";
/// Hardware address of a mapping.
const MAC: &str = "mac";
/// Fixed address of a mapping.
const IPADDR: &str = "ipaddr";
/// Hostname of a mapping.
const HOSTNAME: &str = "hostname";

/// Why a pfSense configuration's static mappings could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseStaticMapError {
    /// The file is not UTF-8.
    #[error("not UTF-8 text")]
    NotUtf8,
    /// The file is not well-formed XML; only the position is kept, since
    /// the text around it may be a secret.
    #[error("invalid XML at line {line}, column {column}")]
    Xml { line: u32, column: u32 },
    /// The root element is not `<pfsense>`.
    #[error("root element is <{0}>, not <pfsense>")]
    NotPfSense(String),
    /// A mapping's `<mac>` is not a MAC address.
    #[error("static mapping on {interface}: invalid <mac> {value:?}")]
    InvalidMac { interface: String, value: String },
    /// A mapping's `<ipaddr>` is not an IPv4 address.
    #[error("static mapping on {interface}: invalid <ipaddr> {value:?}")]
    InvalidIp { interface: String, value: String },
}

/// [`parse`] for raw bytes, which must be UTF-8.
///
/// # Errors
///
/// Returns [`ParseStaticMapError::NotUtf8`], or what [`parse`] returns.
pub fn parse_bytes(bytes: &[u8], subnets: &Subnets) -> Result<Vec<Lease>, ParseStaticMapError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ParseStaticMapError::NotUtf8)?;
    parse(text, subnets)
}

/// The static mappings in `text`, a pfSense `config.xml`, in file order,
/// as leases in [`LeaseState::Static`] with no start or end, and
/// interfaces from `subnets`.
///
/// # Errors
///
/// Returns a [`ParseStaticMapError`]; see the module documentation for each
/// case.
pub fn parse(text: &str, subnets: &Subnets) -> Result<Vec<Lease>, ParseStaticMapError> {
    let document = Document::parse(text).map_err(|err| {
        let position = err.pos();
        ParseStaticMapError::Xml {
            line: position.row,
            column: position.col,
        }
    })?;
    let root = document.root_element();
    if root.tag_name().name() != ROOT {
        return Err(ParseStaticMapError::NotPfSense(
            root.tag_name().name().to_owned(),
        ));
    }
    let mut leases = Vec::new();
    for dhcpd in children(root, DHCPD) {
        for interface in dhcpd.children().filter(Node::is_element) {
            for mapping in children(interface, STATICMAP) {
                if let Some(lease) = mapping_lease(interface, mapping, subnets)? {
                    leases.push(lease);
                }
            }
        }
    }
    Ok(leases)
}

/// The element children of `node` named `name`.
fn children<'a, 'input: 'a>(
    node: Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    node.children()
        .filter(move |child| child.is_element() && child.tag_name().name() == name)
}

/// The trimmed text of `node`'s first child named `name`; `None` when it is
/// missing or empty.
fn field<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .and_then(|child| child.text())
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// The lease of one `<staticmap>` under `interface`, or `None` when it has
/// no MAC or no fixed address.
fn mapping_lease(
    interface: Node<'_, '_>,
    mapping: Node<'_, '_>,
    subnets: &Subnets,
) -> Result<Option<Lease>, ParseStaticMapError> {
    let (Some(mac), Some(ip)) = (field(mapping, MAC), field(mapping, IPADDR)) else {
        return Ok(None);
    };
    let interface = interface.tag_name().name();
    let mac: MacAddr = mac.parse().map_err(|_| ParseStaticMapError::InvalidMac {
        interface: interface.to_owned(),
        value: mac.to_owned(),
    })?;
    let ip: Ipv4Addr = ip.parse().map_err(|_| ParseStaticMapError::InvalidIp {
        interface: interface.to_owned(),
        value: ip.to_owned(),
    })?;
    Ok(Some(Lease {
        ip,
        mac,
        hostname: field(mapping, HOSTNAME).map(str::to_owned),
        start: None,
        end: None,
        state: LeaseState::Static,
        interface: subnets.interface_of(ip).map(str::to_owned),
    }))
}

#[cfg(test)]
#[path = "staticmap_test.rs"]
mod tests;

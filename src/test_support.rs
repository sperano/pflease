//! Fixtures shared by the lease-source tests: the recorded lease file, the
//! leases it holds, and a fake `ssh` program.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use jiff::Timestamp;

use crate::{Lease, LeaseState, MacAddr, Subnets};

/// An anonymized ISC 4.4.3-P1 `dhcpd.leases`.
pub(crate) const DHCPD_LEASES: &str = include_str!("../tests/fixtures/dhcpd.leases");

/// A hand-written pfSense `config.xml` with static mappings.
pub(crate) const CONFIG_XML: &str = include_str!("../tests/fixtures/config.xml");

/// Mode of the fake `ssh` script: readable and executable by its owner.
const SCRIPT_MODE: u32 = 0o700;

/// Held while a fake program is written and run: a child forked by another
/// test thread while the script is still open for writing would make its
/// exec fail with "text file busy".
static SPAWN_LOCK: Mutex<()> = Mutex::new(());

/// The subnets the fixture's addresses fall in, but for `203.0.113.5`.
pub(crate) fn fixture_subnets() -> Subnets {
    Subnets::new(vec![
        ("lan".into(), "192.0.2.0/24".parse().expect("valid range")),
        (
            "iot".into(),
            "198.51.100.0/24".parse().expect("valid range"),
        ),
    ])
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("valid RFC 3339 timestamp")
}

fn mac(last: [u8; 2]) -> MacAddr {
    MacAddr::new([0x02, 0, 0, 0, last[0], last[1]])
}

fn lease(
    ip: [u8; 4],
    mac: MacAddr,
    hostname: Option<&str>,
    times: [Option<Timestamp>; 2],
) -> Lease {
    Lease {
        ip: Ipv4Addr::from(ip),
        mac,
        hostname: hostname.map(str::to_owned),
        start: times[0],
        end: times[1],
        state: LeaseState::Active,
        interface: None,
    }
}

/// The leases [`DHCPD_LEASES`] holds with [`fixture_subnets`], written out
/// by hand from the file.
pub(crate) fn fixture_leases() -> Vec<Lease> {
    let lan = Some("lan".to_owned());
    let iot = Some("iot".to_owned());
    vec![
        Lease {
            state: LeaseState::Released,
            interface: lan.clone(),
            ..lease(
                [192, 0, 2, 12],
                mac([0, 0x12]),
                Some("old-laptop"),
                [
                    Some(at("2026-11-20T07:00:00Z")),
                    Some(at("2026-11-20T09:30:00Z")),
                ],
            )
        },
        Lease {
            interface: lan.clone(),
            ..lease(
                [192, 0, 2, 10],
                mac([0, 0x0a]),
                Some("printer"),
                [
                    Some(at("2026-11-22T14:02:11Z")),
                    Some(at("2026-11-22T16:02:11Z")),
                ],
            )
        },
        Lease {
            state: LeaseState::Free,
            interface: lan,
            ..lease(
                [192, 0, 2, 11],
                mac([0, 0x0b]),
                None,
                [
                    Some(at("2026-11-21T09:15:00Z")),
                    Some(at("2026-11-21T11:15:00Z")),
                ],
            )
        },
        Lease {
            interface: iot.clone(),
            ..lease(
                [198, 51, 100, 20],
                mac([1, 0x14]),
                Some("thermostat"),
                [
                    Some(at("2026-11-22T13:00:05Z")),
                    Some(at("2026-11-22T15:00:05Z")),
                ],
            )
        },
        Lease {
            interface: iot,
            ..lease(
                [198, 51, 100, 21],
                mac([1, 0x15]),
                Some("nas"),
                [Some(at("2026-11-01T00:00:00Z")), None],
            )
        },
        Lease {
            state: LeaseState::Expired,
            ..lease(
                [203, 0, 113, 5],
                mac([2, 0x05]),
                None,
                [
                    Some(at("2026-11-18T06:30:00Z")),
                    Some(at("2026-11-18T08:30:00Z")),
                ],
            )
        },
    ]
}

/// The static mappings of [`CONFIG_XML`] with [`fixture_subnets`]: those
/// with a MAC and an address, IPv6 ones left out.
pub(crate) fn fixture_mappings() -> Vec<Lease> {
    let mapping =
        |ip: Ipv4Addr, mac: [u8; 2], hostname: Option<&str>, interface: Option<&str>| Lease {
            ip,
            mac: self::mac(mac),
            hostname: hostname.map(str::to_owned),
            start: None,
            end: None,
            state: LeaseState::Static,
            interface: interface.map(str::to_owned),
        };
    vec![
        mapping(
            Ipv4Addr::new(192, 0, 2, 30),
            [0x00, 0x30],
            Some("camera"),
            Some("lan"),
        ),
        mapping(
            Ipv4Addr::new(198, 51, 100, 40),
            [0x01, 0x40],
            None,
            Some("iot"),
        ),
        mapping(
            Ipv4Addr::new(203, 0, 113, 7),
            [0x02, 0x07],
            Some("guest-tv"),
            None,
        ),
    ]
}

/// Holds [`SPAWN_LOCK`]; take it around any spawn in a test, a fake
/// program's or not.
pub(crate) fn spawn_lock() -> MutexGuard<'static, ()> {
    SPAWN_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A fake program in `dir` running the POSIX shell `body`; hold the
/// returned guard while the program runs.
pub(crate) fn fake_program(dir: &Path, body: &str) -> (PathBuf, MutexGuard<'static, ()>) {
    use std::os::unix::fs::PermissionsExt;

    let guard = spawn_lock();
    let path = dir.join("fake-ssh");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write fake program");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(SCRIPT_MODE))
        .expect("make fake program executable");
    (path, guard)
}

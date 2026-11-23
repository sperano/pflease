use std::path::Path;
use std::time::Duration;

use super::{IscSshSource, LeaseSource, SourceError};
use crate::Subnets;
use crate::config::Settings;
use crate::isc::ErrorKind;
use crate::ssh::{Ssh, SshError};
use crate::test_support::{fake_program, fixture_leases, fixture_subnets};

/// Timeout for fake programs, which all finish.
const GENEROUS: Duration = Duration::from_secs(30);

fn settings() -> Settings {
    Settings {
        host: "fw.example".into(),
        user: None,
        port: None,
        identity_file: None,
        lease_file: "/var/dhcpd/var/db/dhcpd.leases".into(),
        interfaces: fixture_subnets(),
    }
}

/// The leases of a source whose fake `ssh` runs `body`.
fn leases_with(body: &str) -> Result<Vec<crate::Lease>, SourceError> {
    let dir = tempfile::tempdir().expect("temp dir");
    let (program, _guard) = fake_program(dir.path(), body);
    let settings = settings();
    let ssh = Ssh::new(&settings)
        .with_program(program)
        .with_timeout(GENEROUS);
    IscSshSource::with_ssh(ssh, settings.lease_file, settings.interfaces).leases()
}

#[test]
fn reads_and_parses_the_lease_file_over_ssh() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dhcpd.leases");
    let body = format!("exec cat '{}'", fixture.display());
    assert_eq!(leases_with(&body).expect("reads"), fixture_leases());
}

#[test]
fn ssh_failure_reaches_the_caller() {
    let err = leases_with("echo 'Connection timed out' >&2; exit 255").expect_err("fails");
    assert!(
        matches!(&err, SourceError::Ssh(SshError::Connect { .. })),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "ssh to fw.example failed: Connection timed out"
    );
}

#[test]
fn unparsable_file_names_the_file_and_line() {
    let err = leases_with("printf 'lease 192.0.2.1 {\\n  starts soon;\\n}\\n'").expect_err("fails");
    let SourceError::Parse { path, source } = &err else {
        panic!("not a parse error: {err:?}");
    };
    assert_eq!(path, "/var/dhcpd/var/db/dhcpd.leases");
    assert_eq!(source.line(), 2);
    assert_eq!(
        err.to_string(),
        "cannot parse lease file /var/dhcpd/var/db/dhcpd.leases: \
         line 2: invalid `starts` value \"soon\""
    );
}

#[test]
fn file_that_is_not_utf8_is_a_parse_error() {
    let err = leases_with("printf '\\377'").expect_err("fails");
    assert!(
        matches!(&err, SourceError::Parse { source, .. } if source.kind() == &ErrorKind::NotUtf8),
        "{err:?}"
    );
}

#[test]
fn new_takes_the_lease_file_and_subnets_from_settings() {
    let settings = Settings {
        interfaces: Subnets::default(),
        ..settings()
    };
    let source = IscSshSource::new(&settings);
    assert_eq!(source.lease_file, settings.lease_file);
    assert_eq!(source.subnets, Subnets::default());
    assert_eq!(source.ssh, Ssh::new(&settings));
}

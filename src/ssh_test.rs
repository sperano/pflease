use std::ffi::OsString;
use std::io;
use std::num::NonZeroU16;
use std::time::Duration;

use super::{CONNECT_TIMEOUT, MAX_STDERR, NO_MESSAGE, Ssh, SshError, message, shell_quote};
use crate::Subnets;
use crate::config::Settings;
use crate::test_support::fake_program;

/// Timeout for fake programs that are meant to finish.
const GENEROUS: Duration = Duration::from_secs(30);
/// Timeout for the fake program that never finishes.
const SHORT: Duration = Duration::from_millis(200);
/// Port in the argument tests.
const TEST_PORT: u16 = 2222;

fn settings() -> Settings {
    Settings {
        host: "fw.example".into(),
        user: None,
        port: None,
        identity_file: None,
        lease_file: "/var/db/leases".into(),
        interfaces: Subnets::default(),
    }
}

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// `settings()` reached through a fake program running `body`.
fn read_with(body: &str, timeout: Duration) -> Result<Vec<u8>, SshError> {
    let dir = tempfile::tempdir().expect("temp dir");
    let (program, _guard) = fake_program(dir.path(), body);
    Ssh::new(&settings())
        .with_program(program)
        .with_timeout(timeout)
        .read_file("/var/db/it's leases")
}

#[test]
fn minimal_args_leave_user_port_and_key_to_ssh() {
    let connect = format!("ConnectTimeout={}", CONNECT_TIMEOUT.as_secs());
    assert_eq!(
        Ssh::new(&settings()).args("cat x"),
        os(&[
            "-o",
            "BatchMode=yes",
            "-o",
            &connect,
            "--",
            "fw.example",
            "cat x"
        ])
    );
}

#[test]
fn user_port_and_identity_file_become_options() {
    let settings = Settings {
        user: Some("leases".into()),
        port: NonZeroU16::new(TEST_PORT),
        identity_file: Some("/keys/fw".into()),
        ..settings()
    };
    let args = Ssh::new(&settings).args("cat x");
    assert_eq!(
        args[4..],
        os(&[
            "-l",
            "leases",
            "-p",
            "2222",
            "-i",
            "/keys/fw",
            "--",
            "fw.example",
            "cat x"
        ])
    );
}

#[test]
fn shell_quote_survives_single_quotes() {
    assert_eq!(shell_quote("/a b/it's"), r"'/a b/it'\''s'");
}

#[test]
fn read_file_returns_stdout_and_passes_the_quoted_cat_command() {
    let out = read_with(r#"printf '%s\n' "$@""#, GENEROUS).expect("reads");
    let connect = format!("ConnectTimeout={}", CONNECT_TIMEOUT.as_secs());
    let expected = ["-o", "BatchMode=yes", "-o", &connect, "--", "fw.example"]
        .iter()
        .flat_map(|arg| [*arg, "\n"])
        .collect::<String>()
        + "cat -- '/var/db/it'\\''s leases'\n";
    assert_eq!(String::from_utf8(out).expect("utf-8"), expected);
}

#[test]
fn exit_255_is_an_ssh_failure_with_its_message() {
    let err = read_with(
        "echo 'Permission denied (publickey).' >&2; exit 255",
        GENEROUS,
    )
    .expect_err("refused");
    assert!(
        matches!(&err, SshError::Connect { host, stderr }
            if host == "fw.example" && stderr == "Permission denied (publickey)."),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "ssh to fw.example failed: Permission denied (publickey)."
    );
}

#[test]
fn other_exit_status_is_a_remote_failure() {
    let err = read_with(
        "echo 'cat: x: No such file or directory' >&2; exit 1",
        GENEROUS,
    )
    .expect_err("missing file");
    let SshError::Remote {
        host,
        path,
        status,
        stderr,
    } = &err
    else {
        panic!("not a remote failure: {err:?}");
    };
    assert_eq!(
        (host.as_str(), path.as_str(), status.code(), stderr.as_str()),
        (
            "fw.example",
            "/var/db/it's leases",
            Some(1),
            "cat: x: No such file or directory"
        )
    );
    assert_eq!(
        err.to_string(),
        "cannot read /var/db/it's leases on fw.example (exit status: 1): \
         cat: x: No such file or directory"
    );
}

#[test]
fn silent_failure_says_there_was_no_message() {
    let err = read_with("exit 255", GENEROUS).expect_err("fails");
    assert!(
        matches!(&err, SshError::Connect { stderr, .. } if stderr == NO_MESSAGE),
        "{err:?}"
    );
}

#[test]
fn hung_ssh_is_killed_after_the_timeout() {
    let err = read_with("exec sleep 30", SHORT).expect_err("times out");
    assert!(
        matches!(&err, SshError::TimedOut { host, timeout } if host == "fw.example" && *timeout == SHORT),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "ssh to fw.example did not finish within 0.2 s"
    );
}

#[test]
fn missing_program_is_a_spawn_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("no-ssh");
    let err = Ssh::new(&settings())
        .with_program(&missing)
        .read_file("/x")
        .expect_err("cannot start");
    let SshError::Spawn { program, source } = &err else {
        panic!("not a spawn error: {err:?}");
    };
    assert_eq!(program, &missing.display().to_string());
    assert_eq!(source.kind(), io::ErrorKind::NotFound);
}

#[test]
fn message_escapes_control_characters_but_keeps_newlines() {
    assert_eq!(message(b"  a\x1b[31mb\nc\r\n"), "a\\u{1b}[31mb\nc");
}

#[test]
fn message_keeps_at_most_max_stderr_bytes() {
    let long = vec![b'x'; MAX_STDERR + 1];
    assert_eq!(message(&long), "x".repeat(MAX_STDERR));
}

#[test]
fn blank_message_is_no_message() {
    assert_eq!(message(b" \n\t"), NO_MESSAGE);
}

use std::ffi::OsString;
use std::io;
use std::num::NonZeroU16;
use std::time::{Duration, Instant};

use super::{
    CONNECT_TIMEOUT, MAX_FILE_SIZE, MAX_STDERR, NO_MESSAGE, Ssh, SshError, message, shell_quote,
};
use crate::Subnets;
use crate::config::Settings;
use crate::test_support::{fake_program, spawn_lock};

/// Timeout for fake programs that are meant to finish.
const GENEROUS: Duration = Duration::from_secs(30);
/// Timeout for the fake program that never finishes.
const SHORT: Duration = Duration::from_millis(200);
/// Seconds a hung fake program sleeps: past [`GENEROUS`], so a read that
/// waited for it instead of killing it shows.
const HUNG_SECONDS: u64 = 60;
/// Port in the argument tests.
const TEST_PORT: u16 = 2222;
/// Output well past a pipe buffer (64 KiB on Linux), which only a drained
/// pipe lets through.
const LARGE_OUTPUT: usize = 256 * 1024;
/// File size limit in the size tests.
const SMALL_LIMIT: usize = 16;

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
    read_limited(body, timeout, MAX_FILE_SIZE)
}

/// [`read_with`], refusing files larger than `limit`.
fn read_limited(body: &str, timeout: Duration, limit: usize) -> Result<Vec<u8>, SshError> {
    let dir = tempfile::tempdir().expect("temp dir");
    let (program, _guard) = fake_program(dir.path(), body);
    Ssh::new(&settings())
        .with_program(program)
        .with_timeout(timeout)
        .with_max_file_size(limit)
        .read_file("/var/db/it's leases")
}

fn assert_timed_out(err: &SshError) {
    assert!(
        matches!(err, SshError::TimedOut { host, timeout } if host == "fw.example" && *timeout == SHORT),
        "{err:?}"
    );
}

/// Whether process `pid` still exists, by `kill -0`.
fn process_exists(pid: &str) -> bool {
    let _guard = spawn_lock();
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .expect("run kill")
        .success()
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

/// Kills the process `pid`, which the test started in the background.
fn kill(pid: &str) {
    let _guard = spawn_lock();
    let status = std::process::Command::new("kill")
        .arg(pid)
        .status()
        .expect("run kill");
    assert!(status.success(), "kill {pid} failed");
}

#[test]
fn hung_ssh_is_killed_after_the_timeout() {
    let pid_dir = tempfile::tempdir().expect("temp dir");
    let pid_file = pid_dir.path().join("pid");
    // The script writes its pid, then becomes the `sleep` that hangs.
    let body = format!(
        "echo $$ > '{}'; exec sleep {HUNG_SECONDS}",
        pid_file.display()
    );
    let started = Instant::now();
    let err = read_with(&body, SHORT).expect_err("times out");
    assert!(started.elapsed() < GENEROUS, "waited for the hung ssh");
    assert_timed_out(&err);
    let pid = std::fs::read_to_string(&pid_file).expect("fake wrote its pid");
    assert!(!process_exists(pid.trim()), "ssh {pid} still runs");
    assert_eq!(
        err.to_string(),
        "ssh to fw.example did not finish within 0.2 s"
    );
}

#[test]
fn pipe_held_open_after_exit_times_out() {
    // A background child keeps stdout open after the script exits 0, like
    // a `ProxyCommand` outliving `ssh`. It writes its pid so the test can
    // end it instead of leaving it, and the drain threads, running.
    let pid_dir = tempfile::tempdir().expect("temp dir");
    let pid_file = pid_dir.path().join("pid");
    let body = format!(
        "sleep {HUNG_SECONDS} & echo $! > '{}'; echo partial",
        pid_file.display()
    );
    let err = read_with(&body, SHORT).expect_err("times out");
    let pid = std::fs::read_to_string(&pid_file).expect("fake wrote its pid");
    kill(pid.trim());
    assert_timed_out(&err);
}

#[test]
fn output_larger_than_a_pipe_buffer_is_read_whole() {
    let body = format!("head -c {LARGE_OUTPUT} /dev/zero");
    let out = read_with(&body, GENEROUS).expect("reads");
    assert_eq!(out, vec![0; LARGE_OUTPUT]);
}

#[test]
fn file_of_exactly_the_limit_is_read() {
    let body = format!("head -c {SMALL_LIMIT} /dev/zero");
    let out = read_limited(&body, GENEROUS, SMALL_LIMIT).expect("reads");
    assert_eq!(out, vec![0; SMALL_LIMIT]);
}

#[test]
fn file_over_the_limit_is_refused() {
    // Endless output: the refusal must not wait for the deadline either.
    let err = read_limited("exec cat /dev/zero", GENEROUS, SMALL_LIMIT).expect_err("too large");
    assert!(
        matches!(&err, SshError::TooLarge { host, path, limit }
            if host == "fw.example" && path == "/var/db/it's leases" && *limit == SMALL_LIMIT),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "/var/db/it's leases on fw.example is larger than 16 bytes"
    );
}

#[test]
fn stderr_past_its_limit_does_not_block_ssh() {
    let body = format!("head -c {LARGE_OUTPUT} /dev/zero | tr '\\0' x >&2; exit 255");
    let err = read_with(&body, GENEROUS).expect_err("fails");
    assert!(
        matches!(&err, SshError::Connect { stderr, .. } if *stderr == "x".repeat(MAX_STDERR)),
        "{err:?}"
    );
}

#[test]
fn missing_program_is_a_spawn_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("no-ssh");
    let _guard = spawn_lock();
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

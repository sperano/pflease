//! End-to-end tests of `pflease list`: the real binary, its config loading
//! and its SSH source, with a fake `ssh` first on `PATH` standing in for
//! the firewall.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, PoisonError};

use tempfile::TempDir;

/// Mode of the fake `ssh` script: readable and executable by its owner.
const SCRIPT_MODE: u32 = 0o700;
/// The exit status of a failed `pflease` run.
const FAILURE: i32 = 1;
/// Where the fake `ssh` records its arguments, one per line.
const ARGS_FILE: &str = "ssh-args";

/// Held while a fake `ssh` is written and run: a child forked by another
/// test thread while the script is still open for writing would make its
/// exec fail with "text file busy".
static SPAWN_LOCK: Mutex<()> = Mutex::new(());

/// The recorded lease database the fake `ssh` serves.
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dhcpd.leases")
}

/// A config naming the fixture's two subnets.
const CONFIG: &str = r#"
host = "fw.example"

[interfaces]
lan = "192.0.2.0/24"
iot = "198.51.100.0/24"
"#;

/// A scratch home with a `bin/ssh` that records its arguments and runs
/// `body`, and the config file `config`.
struct Firewall {
    dir: TempDir,
    _guard: MutexGuard<'static, ()>,
}

impl Firewall {
    fn new(body: &str, config: &str) -> Self {
        let guard = SPAWN_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let dir = tempfile::tempdir().expect("temp dir");
        let bin = dir.path().join("bin");
        fs::create_dir(&bin).expect("bin dir");
        let ssh = bin.join("ssh");
        let args = dir.path().join(ARGS_FILE);
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n{body}\n",
            args.display()
        );
        fs::write(&ssh, script).expect("write fake ssh");
        fs::set_permissions(&ssh, fs::Permissions::from_mode(SCRIPT_MODE))
            .expect("make fake ssh executable");
        let config_dir = dir.path().join("config/pflease");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(config_dir.join("config.toml"), config).expect("write config");
        Self { dir, _guard: guard }
    }

    /// A fake `ssh` that serves the fixture.
    fn serving_fixture() -> Self {
        Self::new(&format!("exec cat '{}'", fixture().display()), CONFIG)
    }

    /// Runs `pflease args` with only this home's settings and `ssh`.
    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut path = OsString::from(self.dir.path().join("bin"));
        path.push(":/usr/bin:/bin");
        Command::new(env!("CARGO_BIN_EXE_pflease"))
            .args(args)
            .env_clear()
            .env("PATH", path)
            .env("HOME", self.dir.path())
            .env("XDG_CONFIG_HOME", self.dir.path().join("config"))
            .envs(env.iter().copied())
            .output()
            .expect("run pflease")
    }

    /// The arguments the fake `ssh` was given.
    fn ssh_args(&self) -> Vec<String> {
        fs::read_to_string(self.dir.path().join(ARGS_FILE))
            .expect("ssh was run")
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn stdout(output: &Output) -> &str {
    std::str::from_utf8(&output.stdout).expect("UTF-8 stdout")
}

fn stderr(output: &Output) -> &str {
    std::str::from_utf8(&output.stderr).expect("UTF-8 stderr")
}

/// Asserts `output` is a failure that printed only `message` to stderr.
fn assert_fails_with(output: &Output, message: &str) {
    assert_eq!(output.status.code(), Some(FAILURE), "{output:?}");
    assert_eq!(stdout(output), "");
    assert_eq!(stderr(output), format!("pflease: {message}\n"));
}

#[test]
fn list_shows_the_firewall_leases() {
    let firewall = Firewall::serving_fixture();
    let output = firewall.run(&["list", "--utc"], &[]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(stderr(&output), "");
    assert_eq!(
        stdout(&output),
        "\
IP             MAC                HOSTNAME    START                END                  STATE     INTERFACE
192.0.2.10     02:00:00:00:00:0a  printer     2026-11-22 14:02:11  2026-11-22 16:02:11  active    lan
192.0.2.11     02:00:00:00:00:0b  -           2026-11-21 09:15:00  2026-11-21 11:15:00  free      lan
192.0.2.12     02:00:00:00:00:12  old-laptop  2026-11-20 07:00:00  2026-11-20 09:30:00  released  lan
198.51.100.20  02:00:00:00:01:14  thermostat  2026-11-22 13:00:05  2026-11-22 15:00:05  active    iot
198.51.100.21  02:00:00:00:01:15  nas         2026-11-01 00:00:00  -                    active    iot
203.0.113.5    02:00:00:00:02:05  -           2026-11-18 06:30:00  2026-11-18 08:30:00  expired   -
"
    );
}

#[test]
fn list_reads_the_default_lease_file_on_the_configured_host() {
    let firewall = Firewall::serving_fixture();
    let output = firewall.run(&["list"], &[]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        firewall.ssh_args(),
        [
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "--",
            "fw.example",
            "cat -- '/var/dhcpd/var/db/dhcpd.leases'",
        ]
    );
}

#[test]
fn list_passes_environment_and_flag_settings_to_ssh() {
    let firewall = Firewall::serving_fixture();
    let output = firewall.run(
        &["list", "--port", "2222", "--lease-file", "/tmp/it's.leases"],
        &[("PFLEASE_HOST", "fw.env"), ("PFLEASE_USER", "leases")],
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        firewall.ssh_args(),
        [
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-l",
            "leases",
            "-p",
            "2222",
            "--",
            "fw.env",
            r"cat -- '/tmp/it'\''s.leases'",
        ]
    );
}

#[test]
fn list_applies_filter_and_sort_to_the_firewall_leases() {
    let firewall = Firewall::serving_fixture();
    let output = firewall.run(
        &["list", "--utc", "--interface", "iot", "--sort", "hostname"],
        &[],
    );
    assert!(output.status.success(), "{output:?}");
    let hosts: Vec<&str> = stdout(&output)
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().nth(2).expect("hostname column"))
        .collect();
    assert_eq!(hosts, ["nas", "thermostat"]);
}

#[test]
fn list_without_host_fails_before_running_ssh() {
    let firewall = Firewall::new("exit 0", "");
    let output = firewall.run(&["list"], &[]);
    let config = firewall.dir.path().join("config/pflease/config.toml");
    assert_fails_with(
        &output,
        &format!(
            "missing setting `host`: set `host` in {}, the PFLEASE_HOST \
             environment variable, or --host",
            config.display()
        ),
    );
    assert!(!firewall.dir.path().join(ARGS_FILE).exists());
}

#[test]
fn list_reports_an_ssh_failure() {
    let firewall = Firewall::new(
        "echo 'Permission denied (publickey).' >&2; exit 255",
        CONFIG,
    );
    let output = firewall.run(&["list"], &[]);
    assert_fails_with(
        &output,
        "ssh to fw.example failed: Permission denied (publickey).",
    );
}

#[test]
fn list_reports_a_missing_lease_file() {
    let firewall = Firewall::new(
        "echo 'cat: /var/dhcpd/var/db/dhcpd.leases: No such file or directory' >&2; exit 1",
        CONFIG,
    );
    let output = firewall.run(&["list"], &[]);
    assert_fails_with(
        &output,
        "cannot read /var/dhcpd/var/db/dhcpd.leases on fw.example (exit status: 1): \
         cat: /var/dhcpd/var/db/dhcpd.leases: No such file or directory",
    );
}

#[test]
fn list_reports_an_unparsable_lease_file() {
    let firewall = Firewall::new("printf 'lease 192.0.2.1 {\\n  starts soon;\\n}\\n'", CONFIG);
    let output = firewall.run(&["list"], &[]);
    assert_fails_with(
        &output,
        "cannot parse lease file /var/dhcpd/var/db/dhcpd.leases: line 2: \
         invalid `starts` value \"soon\"",
    );
}

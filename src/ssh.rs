//! Transport: reads a file on the firewall by running the user's `ssh`, so
//! its config, agent and `known_hosts` apply.
//!
//! `ssh` runs in batch mode (it never prompts) with a connect timeout, and
//! the whole read has a deadline after which `ssh` is killed. Its exit
//! status tells the failures apart: 255 is `ssh`'s own (host unreachable,
//! authentication refused, host key mismatch), anything else is the remote
//! command's (file missing or unreadable).

use std::ffi::OsString;
use std::io::{self, Read};
use std::num::NonZeroU16;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config::Settings;

/// How long `ssh` may take to open the connection (`ConnectTimeout`).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a whole read may take, connection included.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// How often a running `ssh` is checked for exit.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// The exit status `ssh` uses for its own errors.
const SSH_FAILURE: i32 = 255;
/// Most bytes of `ssh`'s standard error kept in an error message.
const MAX_STDERR: usize = 4096;
/// The program run by default.
const SSH_PROGRAM: &str = "ssh";
/// What an error shows when `ssh` wrote nothing to standard error.
const NO_MESSAGE: &str = "no error message";

/// Why a file could not be read over SSH.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SshError {
    /// The `ssh` program could not be started.
    #[error("cannot run {program}: {source}")]
    Spawn { program: String, source: io::Error },
    /// `ssh` itself failed (exit 255): unreachable host, refused
    /// authentication, host key mismatch.
    #[error("ssh to {host} failed: {stderr}")]
    Connect { host: String, stderr: String },
    /// The remote command failed.
    #[error("cannot read {path} on {host} ({status}): {stderr}")]
    Remote {
        host: String,
        path: String,
        status: ExitStatus,
        stderr: String,
    },
    /// `ssh` did not finish in time and was killed.
    #[error("ssh to {host} did not finish within {} s", timeout.as_secs_f64())]
    TimedOut { host: String, timeout: Duration },
    /// Waiting for `ssh` or reading its output failed.
    #[error("lost track of ssh to {host}: {source}")]
    Io { host: String, source: io::Error },
}

/// How to reach the firewall with `ssh`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ssh {
    program: OsString,
    host: String,
    user: Option<String>,
    port: Option<NonZeroU16>,
    identity_file: Option<PathBuf>,
    timeout: Duration,
}

impl Ssh {
    /// Runs `ssh` with the SSH part of `settings` and [`READ_TIMEOUT`].
    #[must_use]
    pub fn new(settings: &Settings) -> Self {
        Self {
            program: SSH_PROGRAM.into(),
            host: settings.host.clone(),
            user: settings.user.clone(),
            port: settings.port,
            identity_file: settings.identity_file.clone(),
            timeout: READ_TIMEOUT,
        }
    }

    /// Runs `program` instead of `ssh`, with the same arguments.
    #[must_use]
    pub fn with_program(mut self, program: impl Into<OsString>) -> Self {
        self.program = program.into();
        self
    }

    /// Gives up on a read after `timeout` instead of [`READ_TIMEOUT`].
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The arguments that run `remote_command` on the firewall.
    #[must_use]
    pub fn args(&self, remote_command: &str) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            format!("ConnectTimeout={}", CONNECT_TIMEOUT.as_secs()).into(),
        ];
        if let Some(user) = &self.user {
            args.extend(["-l".into(), user.into()]);
        }
        if let Some(port) = self.port {
            args.extend(["-p".into(), port.to_string().into()]);
        }
        if let Some(identity_file) = &self.identity_file {
            args.extend(["-i".into(), identity_file.into()]);
        }
        args.extend(["--".into(), (&self.host).into(), remote_command.into()]);
        args
    }

    /// The bytes of the file at `path` on the firewall.
    ///
    /// # Errors
    ///
    /// Returns an [`SshError`] when `ssh` cannot start, fails, the remote
    /// command fails or the read exceeds its timeout.
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, SshError> {
        let args = self.args(&format!("cat -- {}", shell_quote(path)));
        let (status, stdout, stderr) = self.run(&args)?;
        if status.success() {
            return Ok(stdout);
        }
        let stderr = message(&stderr);
        let host = self.host.clone();
        Err(if status.code() == Some(SSH_FAILURE) {
            SshError::Connect { host, stderr }
        } else {
            SshError::Remote {
                host,
                path: path.to_owned(),
                status,
                stderr,
            }
        })
    }

    /// Runs the program with `args` until it exits or times out.
    fn run(&self, args: &[OsString]) -> Result<(ExitStatus, Vec<u8>, Vec<u8>), SshError> {
        let mut child = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| SshError::Spawn {
                program: self.program.to_string_lossy().into_owned(),
                source,
            })?;
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let io_error = |source| SshError::Io {
            host: self.host.clone(),
            source,
        };
        let Some(status) = wait(&mut child, self.timeout).map_err(io_error)? else {
            // Best effort: the child may have exited since the last poll.
            let _ = child.kill();
            let _ = child.wait();
            return Err(SshError::TimedOut {
                host: self.host.clone(),
                timeout: self.timeout,
            });
        };
        Ok((
            status,
            join(stdout).map_err(io_error)?,
            join(stderr).map_err(io_error)?,
        ))
    }
}

/// `child`'s exit status, or `None` when it runs longer than `timeout`.
fn wait(child: &mut Child, timeout: Duration) -> io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(None);
        }
        thread::sleep(POLL_INTERVAL.min(deadline - now));
    }
}

/// Reads `pipe` to its end on a thread, so neither pipe fills up and
/// blocks the child.
fn drain(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            pipe.read_to_end(&mut bytes)?;
        }
        Ok(bytes)
    })
}

fn join(reader: JoinHandle<io::Result<Vec<u8>>>) -> io::Result<Vec<u8>> {
    reader
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("output reader panicked")))
}

/// `stderr` as one printable message: at most [`MAX_STDERR`] bytes,
/// trimmed, control characters other than newlines escaped so the remote
/// side cannot drive the terminal.
fn message(stderr: &[u8]) -> String {
    let kept = &stderr[..stderr.len().min(MAX_STDERR)];
    let text = String::from_utf8_lossy(kept);
    let text = text.trim();
    if text.is_empty() {
        return NO_MESSAGE.to_owned();
    }
    let mut message = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control() && c != '\n' {
            message.extend(c.escape_default());
        } else {
            message.push(c);
        }
    }
    message
}

/// `text` as one word for a POSIX shell: in single quotes, each `'` closed,
/// escaped and reopened.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
#[path = "ssh_test.rs"]
mod tests;

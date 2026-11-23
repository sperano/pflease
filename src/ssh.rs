//! Transport: reads a file on the firewall by running the user's `ssh`, so
//! its config, agent and `known_hosts` apply.
//!
//! `ssh` runs in batch mode (it never prompts) with a connect timeout, and
//! the whole read, output included, has a deadline after which `ssh` is
//! killed; a file larger than a set size is refused. Its exit
//! status tells the failures apart: 255 is `ssh`'s own (host unreachable,
//! authentication refused, host key mismatch), anything else is the remote
//! command's (file missing or unreadable).

use std::ffi::OsString;
use std::io::{self, Read};
use std::num::NonZeroU16;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Settings;

/// How long `ssh` may take to open the connection (`ConnectTimeout`).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a whole read may take, connection included.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// How often a running `ssh` is checked for exit.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// Largest file [`Ssh::read_file`] reads by default, in bytes.
pub const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;
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
    /// The file is larger than the most pflease reads.
    #[error("{path} on {host} is larger than {limit} bytes")]
    TooLarge {
        host: String,
        path: String,
        limit: usize,
    },
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
    max_file_size: usize,
}

/// What a finished `ssh` left: its status and both outputs.
struct Output {
    status: ExitStatus,
    stdout: Drained,
    stderr: Drained,
}

/// The bytes read from a pipe, at most a limit of them.
struct Drained {
    bytes: Vec<u8>,
    /// The pipe held more than the limit.
    overflowed: bool,
}

/// What a drain does with a pipe once it holds more than its limit.
#[derive(Clone, Copy)]
enum Excess {
    /// Close the pipe, so the writer gets `SIGPIPE`.
    Close,
    /// Read the rest and throw it away, so the writer goes on.
    Discard,
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
            max_file_size: MAX_FILE_SIZE,
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

    /// Refuses files larger than `bytes` instead of [`MAX_FILE_SIZE`].
    #[must_use]
    pub const fn with_max_file_size(mut self, bytes: usize) -> Self {
        self.max_file_size = bytes;
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
        let Output {
            status,
            stdout,
            stderr,
        } = self.run(&args)?;
        let host = self.host.clone();
        if stdout.overflowed {
            return Err(SshError::TooLarge {
                host,
                path: path.to_owned(),
                limit: self.max_file_size,
            });
        }
        if status.success() {
            return Ok(stdout.bytes);
        }
        let stderr = message(&stderr.bytes);
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

    /// Runs the program with `args` until it exits and closes its outputs,
    /// or until the timeout.
    ///
    /// A process that outlives `ssh` and keeps a pipe open (a
    /// `ProxyCommand`, a `ControlPersist` master) holds the read until the
    /// deadline too; its drain thread is then left blocked, which costs a
    /// thread in a process about to report the error and exit.
    fn run(&self, args: &[OsString]) -> Result<Output, SshError> {
        let deadline = Instant::now() + self.timeout;
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
        let stdout = drain(child.stdout.take(), self.max_file_size, Excess::Close);
        let stderr = drain(child.stderr.take(), MAX_STDERR, Excess::Discard);
        let Some(status) = wait(&mut child, deadline).map_err(|source| self.io_error(source))?
        else {
            // Best effort: the child may have exited since the last poll.
            let _ = child.kill();
            let _ = child.wait();
            return Err(self.timed_out());
        };
        Ok(Output {
            status,
            stdout: self.collect(&stdout, deadline)?,
            stderr: self.collect(&stderr, deadline)?,
        })
    }

    /// What `reader`'s drain read, if it finishes by `deadline`.
    fn collect(
        &self,
        reader: &Receiver<io::Result<Drained>>,
        deadline: Instant,
    ) -> Result<Drained, SshError> {
        match reader.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(drained) => drained.map_err(|source| self.io_error(source)),
            Err(RecvTimeoutError::Timeout) => Err(self.timed_out()),
            Err(RecvTimeoutError::Disconnected) => {
                Err(self.io_error(io::Error::other("output reader panicked")))
            }
        }
    }

    fn io_error(&self, source: io::Error) -> SshError {
        SshError::Io {
            host: self.host.clone(),
            source,
        }
    }

    fn timed_out(&self) -> SshError {
        SshError::TimedOut {
            host: self.host.clone(),
            timeout: self.timeout,
        }
    }
}

/// `child`'s exit status, or `None` when it runs past `deadline`.
fn wait(child: &mut Child, deadline: Instant) -> io::Result<Option<ExitStatus>> {
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

/// Reads `pipe` on a thread, so neither pipe fills up and blocks the
/// child, keeping at most `limit` bytes; past them, `excess` closes the
/// pipe (stdout) or reads the rest to its end and discards it (stderr).
fn drain(
    pipe: Option<impl Read + Send + 'static>,
    limit: usize,
    excess: Excess,
) -> Receiver<io::Result<Drained>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let drained = match pipe {
            Some(pipe) => read_limited(pipe, limit, excess),
            None => Ok(Drained {
                bytes: Vec::new(),
                overflowed: false,
            }),
        };
        // The receiver is gone once `run` gave up on this pipe.
        let _ = sender.send(drained);
    });
    receiver
}

/// At most `limit` bytes of `pipe`, then what `excess` says.
fn read_limited(mut pipe: impl Read, limit: usize, excess: Excess) -> io::Result<Drained> {
    let mut bytes = Vec::new();
    // One byte past the limit tells a pipe that holds exactly `limit` from
    // one that holds more.
    let wanted = u64::try_from(limit).map_or(u64::MAX, |limit| limit.saturating_add(1));
    (&mut pipe).take(wanted).read_to_end(&mut bytes)?;
    let overflowed = bytes.len() > limit;
    if overflowed {
        bytes.truncate(limit);
        match excess {
            Excess::Close => drop(pipe),
            Excess::Discard => {
                io::copy(&mut pipe, &mut io::sink())?;
            }
        }
    }
    Ok(Drained { bytes, overflowed })
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

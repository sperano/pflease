//! Where `pflease` finds the firewall: a config file, environment variables
//! and command-line flags, in that order of precedence (the last one wins).
//!
//! Leases are read over SSH (see `AGENTS.md`), and `pflease` shells out to
//! `ssh`, so the private key stays with `ssh` (its agent, `~/.ssh/config`,
//! `identity_file`). No setting holds a secret, and unknown keys such as
//! `password` are rejected, so the file needs no permission check.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Where pfSense's ISC DHCP server keeps its lease database.
pub const DEFAULT_LEASE_FILE: &str = "/var/dhcpd/var/db/dhcpd.leases";
/// Directory under the XDG config directory that holds the config file.
const CONFIG_DIR: &str = "pflease";
/// Name of the config file inside [`CONFIG_DIR`].
const CONFIG_FILE: &str = "config.toml";
/// Config directory under `$HOME` when `XDG_CONFIG_HOME` is unset.
const HOME_CONFIG_DIR: &str = ".config";
/// Prefix of an argument that `ssh` would read as an option.
const OPTION_PREFIX: char = '-';

/// A setting that can come from the file, the environment or a flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Host,
    User,
    Port,
    IdentityFile,
    LeaseFile,
    Firewall,
}

impl Setting {
    /// Its key in the config file.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::User => "user",
            Self::Port => "port",
            Self::IdentityFile => "identity_file",
            Self::LeaseFile => "lease_file",
            Self::Firewall => "firewall",
        }
    }

    /// Its environment variable.
    #[must_use]
    pub const fn env(self) -> &'static str {
        match self {
            Self::Host => "PFLEASE_HOST",
            Self::User => "PFLEASE_USER",
            Self::Port => "PFLEASE_PORT",
            Self::IdentityFile => "PFLEASE_IDENTITY_FILE",
            Self::LeaseFile => "PFLEASE_LEASE_FILE",
            Self::Firewall => "PFLEASE_FIREWALL",
        }
    }

    /// Its command-line flag.
    #[must_use]
    pub const fn flag(self) -> &'static str {
        match self {
            Self::Host => "--host",
            Self::User => "--user",
            Self::Port => "--port",
            Self::IdentityFile => "--identity-file",
            Self::LeaseFile => "--lease-file",
            Self::Firewall => "--firewall",
        }
    }
}

/// Settings from one place; `None` leaves the setting to a lower layer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub host: Option<String>,
    pub user: Option<String>,
    pub port: Option<NonZeroU16>,
    pub identity_file: Option<PathBuf>,
    pub lease_file: Option<String>,
}

impl Layer {
    /// `self` with every empty value unset, so it cannot hide a lower layer's.
    fn without_empty(self) -> Self {
        Self {
            host: self.host.filter(|host| !host.is_empty()),
            user: self.user.filter(|user| !user.is_empty()),
            port: self.port,
            identity_file: self
                .identity_file
                .filter(|path| !path.as_os_str().is_empty()),
            lease_file: self.lease_file.filter(|path| !path.is_empty()),
        }
    }

    /// `self` with every unset setting taken from `lower`.
    fn over(self, lower: Self) -> Self {
        Self {
            host: self.host.or(lower.host),
            user: self.user.or(lower.user),
            port: self.port.or(lower.port),
            identity_file: self.identity_file.or(lower.identity_file),
            lease_file: self.lease_file.or(lower.lease_file),
        }
    }
}

/// The config file: top-level settings, and named firewalls that override
/// them.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    host: Option<String>,
    user: Option<String>,
    port: Option<NonZeroU16>,
    identity_file: Option<PathBuf>,
    lease_file: Option<String>,
    #[serde(default)]
    firewalls: BTreeMap<String, Layer>,
}

/// What the command line says about the configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// `--config`: replaces the default path, and must exist.
    pub config: Option<PathBuf>,
    /// `--firewall`: the named firewall to use.
    pub firewall: Option<String>,
    /// The setting flags.
    pub flags: Layer,
}

/// How to reach the firewall, every layer applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// SSH destination: a hostname, an address or a `~/.ssh/config` alias.
    pub host: String,
    /// SSH user; `None` leaves it to `ssh`.
    pub user: Option<String>,
    /// SSH port; `None` leaves it to `ssh`.
    pub port: Option<NonZeroU16>,
    /// Private key for `ssh -i`; `None` leaves it to `ssh`.
    pub identity_file: Option<PathBuf>,
    /// Path of the lease database on the firewall.
    pub lease_file: String,
}

/// Why the configuration could not be loaded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// The config file exists but cannot be read, or `--config` names a
    /// missing file.
    #[error("cannot read config file {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    /// The config file is not valid TOML, has an unknown key or a value of
    /// the wrong type. Only the parser's message is kept, not its snippet
    /// of the offending line, so the value of an unknown key such as
    /// `password = "..."` is never echoed; a type error on a known key
    /// (which holds no secret) shows that value.
    #[error("invalid config file {}: {message}{}", path.display(), at(*location))]
    Parse {
        path: PathBuf,
        message: String,
        location: Option<Location>,
    },
    /// The selected firewall is not in the config file.
    #[error("{}", unknown_firewall(name, path.as_deref(), known))]
    UnknownFirewall {
        name: String,
        path: Option<PathBuf>,
        known: Vec<String>,
    },
    /// A required setting is set nowhere.
    #[error("{}", missing(*setting, path.as_deref()))]
    Missing {
        setting: Setting,
        path: Option<PathBuf>,
    },
    /// A setting has a value it cannot take.
    #[error("invalid {}: {reason}", setting.key())]
    Invalid { setting: Setting, reason: String },
}

/// A 1-based position in the config file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl Location {
    /// The position of byte `offset` in `text`.
    fn of(text: &str, offset: usize) -> Self {
        let before = text.get(..offset).unwrap_or(text);
        let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
        Self {
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

fn at(location: Option<Location>) -> String {
    location.map_or_else(String::new, |Location { line, column }| {
        format!(" (line {line}, column {column})")
    })
}

/// Where the config file lives, for messages.
struct FileName<'a>(Option<&'a Path>);

impl fmt::Display for FileName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(path) => write!(f, "{}", path.display()),
            None => write!(f, "the config file (--config)"),
        }
    }
}

fn unknown_firewall(name: &str, path: Option<&Path>, known: &[String]) -> String {
    let known = if known.is_empty() {
        "none defined".to_owned()
    } else {
        format!("defined: {}", known.join(", "))
    };
    format!(
        "no firewall `{name}` in {} ({known}); add a [firewalls.{name}] table or drop {}/{}",
        FileName(path),
        Setting::Firewall.flag(),
        Setting::Firewall.env(),
    )
}

fn missing(setting: Setting, path: Option<&Path>) -> String {
    format!(
        "missing setting `{key}`: set `{key}` in {}, the {} environment variable, or {}",
        FileName(path),
        setting.env(),
        setting.flag(),
        key = setting.key(),
    )
}

/// Reads one environment variable; `None` when it is unset.
type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// Loads the settings: the file, then the environment (read with `env`),
/// then `options`. An empty value, in any layer, counts as unset.
///
/// # Errors
///
/// Returns a [`ConfigError`] when the file cannot be read or parsed, the
/// selected firewall is not defined, a value is invalid (a `PFLEASE_*`
/// variable other than `PFLEASE_IDENTITY_FILE` that is not UTF-8
/// included), or `host` is set nowhere.
pub fn load(options: &Options, env: Env<'_>) -> Result<Settings, ConfigError> {
    let env = |name: &str| env(name).filter(|value| !value.is_empty());
    let path = options.config.clone().or_else(|| default_path(&env));
    let file = match &path {
        Some(path) => read_file(path, options.config.is_some())?,
        None => None,
    };
    let firewall = match options.firewall.clone().filter(|name| !name.is_empty()) {
        Some(name) => Some(name),
        None => env_text(&env, Setting::Firewall)?,
    };
    let from_file = file_layer(file, firewall.as_deref(), path.as_deref())?;
    let merged = options
        .flags
        .clone()
        .without_empty()
        .over(env_layer(&env)?)
        .over(from_file);
    finish(merged, path)
}

/// `$XDG_CONFIG_HOME/pflease/config.toml`, else `$HOME/.config/...`; a
/// relative `XDG_CONFIG_HOME` is ignored, as the XDG spec says.
fn default_path(env: Env<'_>) -> Option<PathBuf> {
    let base = env("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| env("HOME").map(|home| Path::new(&home).join(HOME_CONFIG_DIR)))?;
    Some(base.join(CONFIG_DIR).join(CONFIG_FILE))
}

/// The parsed file; `None` when the default file does not exist.
fn read_file(path: &Path, explicit: bool) -> Result<Option<FileConfig>, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound && !explicit => return Ok(None),
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_owned(),
                source,
            });
        }
    };
    toml::from_str(&text)
        .map(Some)
        .map_err(|err| ConfigError::Parse {
            path: path.to_owned(),
            message: err.message().to_owned(),
            location: err.span().map(|span| Location::of(&text, span.start)),
        })
}

/// The file's top-level settings, overridden by the selected firewall's.
fn file_layer(
    file: Option<FileConfig>,
    firewall: Option<&str>,
    path: Option<&Path>,
) -> Result<Layer, ConfigError> {
    let mut file = file.unwrap_or_default();
    let named = match firewall {
        Some(name) => file
            .firewalls
            .remove(name)
            .ok_or_else(|| ConfigError::UnknownFirewall {
                name: name.to_owned(),
                path: path.map(Path::to_owned),
                known: file.firewalls.keys().cloned().collect(),
            })?,
        None => Layer::default(),
    };
    let top = Layer {
        host: file.host,
        user: file.user,
        port: file.port,
        identity_file: file.identity_file,
        lease_file: file.lease_file,
    };
    Ok(named.without_empty().over(top.without_empty()))
}

/// The value of `setting`'s environment variable, which must be UTF-8.
fn env_text(env: Env<'_>, setting: Setting) -> Result<Option<String>, ConfigError> {
    env(setting.env())
        .map(|value| {
            value.into_string().map_err(|_| ConfigError::Invalid {
                setting,
                reason: format!("{} is not valid UTF-8", setting.env()),
            })
        })
        .transpose()
}

/// The settings from the `PFLEASE_*` environment variables.
fn env_layer(env: Env<'_>) -> Result<Layer, ConfigError> {
    let port = env_text(env, Setting::Port)?
        .map(|value| {
            value.parse().map_err(|err| ConfigError::Invalid {
                setting: Setting::Port,
                reason: format!("{} `{value}`: {err}", Setting::Port.env()),
            })
        })
        .transpose()?;
    Ok(Layer {
        host: env_text(env, Setting::Host)?,
        user: env_text(env, Setting::User)?,
        port,
        identity_file: env(Setting::IdentityFile.env()).map(PathBuf::from),
        lease_file: env_text(env, Setting::LeaseFile)?,
    })
}

/// Checks the merged settings, whose empty values are already unset, and
/// fills in the defaults.
fn finish(merged: Layer, path: Option<PathBuf>) -> Result<Settings, ConfigError> {
    let host = merged.host.ok_or(ConfigError::Missing {
        setting: Setting::Host,
        path,
    })?;
    check_not_option(Setting::Host, &host)?;
    let user = merged.user;
    if let Some(user) = &user {
        check_not_option(Setting::User, user)?;
    }
    Ok(Settings {
        host,
        user,
        port: merged.port,
        identity_file: merged.identity_file,
        lease_file: merged
            .lease_file
            .unwrap_or_else(|| DEFAULT_LEASE_FILE.to_owned()),
    })
}

/// Refuses a value `ssh` would parse as an option (`-oProxyCommand=...`).
fn check_not_option(setting: Setting, value: &str) -> Result<(), ConfigError> {
    if value.starts_with(OPTION_PREFIX) {
        return Err(ConfigError::Invalid {
            setting,
            reason: format!("`{value}` starts with `{OPTION_PREFIX}`"),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "config_test.rs"]
mod tests;

#[cfg(test)]
#[path = "config_env_test.rs"]
mod env_tests;

use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::Subnets;

use super::{ConfigError, DEFAULT_LEASE_FILE, Layer, Location, Options, Setting, Settings, load};

/// A config file with top-level settings and two named firewalls.
const FULL_FILE: &str = r#"
host = "file.example"
user = "file-user"
port = 2201
identity_file = "/keys/file"
lease_file = "/file/dhcpd.leases"

[firewalls.lab]
host = "lab.example"
port = 2202

[firewalls.office]
host = "office.example"
"#;

/// Port set by the environment in the precedence tests.
const ENV_PORT: u16 = 2203;
/// Port set by a flag in the precedence tests.
const FLAG_PORT: u16 = 2204;

pub(super) fn port(value: u16) -> NonZeroU16 {
    NonZeroU16::new(value).expect("test ports are non-zero")
}

/// A temporary directory holding `config.toml` with `contents`.
pub(super) fn file(contents: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, contents).expect("write config");
    (dir, path)
}

pub(super) fn with_config(path: &Path) -> Options {
    Options {
        config: Some(path.to_owned()),
        ..Options::default()
    }
}

/// Loads with an environment made of `vars`.
pub(super) fn load_with(options: &Options, vars: &[(&str, &str)]) -> Result<Settings, ConfigError> {
    let vars: HashMap<String, OsString> = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
        .collect();
    load(options, &|name| vars.get(name).cloned())
}

fn all_env() -> Vec<(&'static str, &'static str)> {
    vec![
        ("PFLEASE_HOST", "env.example"),
        ("PFLEASE_USER", "env-user"),
        ("PFLEASE_PORT", "2203"),
        ("PFLEASE_IDENTITY_FILE", "/keys/env"),
        ("PFLEASE_LEASE_FILE", "/env/dhcpd.leases"),
    ]
}

fn all_flags() -> Layer {
    Layer {
        host: Some("flag.example".into()),
        user: Some("flag-user".into()),
        port: Some(port(FLAG_PORT)),
        identity_file: Some("/keys/flag".into()),
        lease_file: Some("/flag/dhcpd.leases".into()),
        interfaces: None,
    }
}

#[test]
fn file_settings_apply_alone() {
    let (_dir, path) = file(FULL_FILE);
    let settings = load_with(&with_config(&path), &[]).expect("loads");
    assert_eq!(
        settings,
        Settings {
            host: "file.example".into(),
            user: Some("file-user".into()),
            port: Some(port(2201)),
            identity_file: Some("/keys/file".into()),
            lease_file: "/file/dhcpd.leases".into(),
            interfaces: Subnets::default(),
        }
    );
}

#[test]
fn env_overrides_file() {
    let (_dir, path) = file(FULL_FILE);
    let settings = load_with(&with_config(&path), &all_env()).expect("loads");
    assert_eq!(
        settings,
        Settings {
            host: "env.example".into(),
            user: Some("env-user".into()),
            port: Some(port(ENV_PORT)),
            identity_file: Some("/keys/env".into()),
            lease_file: "/env/dhcpd.leases".into(),
            interfaces: Subnets::default(),
        }
    );
}

#[test]
fn flags_override_env_and_file() {
    let (_dir, path) = file(FULL_FILE);
    let options = Options {
        flags: all_flags(),
        ..with_config(&path)
    };
    let settings = load_with(&options, &all_env()).expect("loads");
    assert_eq!(
        settings,
        Settings {
            host: "flag.example".into(),
            user: Some("flag-user".into()),
            port: Some(port(FLAG_PORT)),
            identity_file: Some("/keys/flag".into()),
            lease_file: "/flag/dhcpd.leases".into(),
            interfaces: Subnets::default(),
        }
    );
}

#[test]
fn layers_fill_in_settings_the_higher_ones_leave_unset() {
    let (_dir, path) = file("user = \"file-user\"\n");
    let options = Options {
        flags: Layer {
            port: Some(port(FLAG_PORT)),
            ..Layer::default()
        },
        ..with_config(&path)
    };
    let settings = load_with(&options, &[("PFLEASE_HOST", "env.example")]).expect("loads");
    assert_eq!(settings.host, "env.example");
    assert_eq!(settings.user.as_deref(), Some("file-user"));
    assert_eq!(settings.port, Some(port(FLAG_PORT)));
}

#[test]
fn optional_settings_default_to_ssh_and_the_isc_lease_file() {
    let settings = load_with(&Options::default(), &[("PFLEASE_HOST", "h")]).expect("loads");
    assert_eq!(
        settings,
        Settings {
            host: "h".into(),
            user: None,
            port: None,
            identity_file: None,
            lease_file: DEFAULT_LEASE_FILE.into(),
            interfaces: Subnets::default(),
        }
    );
}

#[test]
fn firewall_flag_selects_a_named_firewall_over_the_top_level() {
    let (_dir, path) = file(FULL_FILE);
    let options = Options {
        firewall: Some("lab".into()),
        ..with_config(&path)
    };
    let settings = load_with(&options, &[]).expect("loads");
    assert_eq!(settings.host, "lab.example");
    assert_eq!(settings.port, Some(port(2202)));
    // Unset in [firewalls.lab], so taken from the top level.
    assert_eq!(settings.user.as_deref(), Some("file-user"));
}

#[test]
fn firewall_env_selects_a_named_firewall() {
    let (_dir, path) = file(FULL_FILE);
    let settings =
        load_with(&with_config(&path), &[("PFLEASE_FIREWALL", "office")]).expect("loads");
    assert_eq!(settings.host, "office.example");
}

#[test]
fn firewall_flag_overrides_firewall_env() {
    let (_dir, path) = file(FULL_FILE);
    let options = Options {
        firewall: Some("lab".into()),
        ..with_config(&path)
    };
    let settings = load_with(&options, &[("PFLEASE_FIREWALL", "office")]).expect("loads");
    assert_eq!(settings.host, "lab.example");
}

#[test]
fn env_overrides_the_selected_firewall() {
    let (_dir, path) = file(FULL_FILE);
    let options = Options {
        firewall: Some("lab".into()),
        ..with_config(&path)
    };
    let settings = load_with(&options, &[("PFLEASE_HOST", "env.example")]).expect("loads");
    assert_eq!(settings.host, "env.example");
}

#[test]
fn unknown_firewall_names_the_defined_ones() {
    let (_dir, path) = file(FULL_FILE);
    let options = Options {
        firewall: Some("home".into()),
        ..with_config(&path)
    };
    let err = load_with(&options, &[]).expect_err("home is not defined");
    assert_eq!(
        err.to_string(),
        format!(
            "no firewall `home` in {} (defined: lab, office); add a [firewalls.home] table \
             or drop --firewall/PFLEASE_FIREWALL",
            path.display()
        )
    );
}

#[test]
fn unknown_firewall_without_a_file_says_none_are_defined() {
    let options = Options {
        firewall: Some("home".into()),
        ..Options::default()
    };
    let err = load_with(&options, &[]).expect_err("no file");
    assert_eq!(
        err.to_string(),
        "no firewall `home` in the config file (--config) (none defined); add a \
         [firewalls.home] table or drop --firewall/PFLEASE_FIREWALL"
    );
}

#[test]
fn missing_host_names_the_file_the_env_var_and_the_flag() {
    let (_dir, path) = file("user = \"admin\"\n");
    let err = load_with(&with_config(&path), &[]).expect_err("no host");
    assert!(matches!(
        err,
        ConfigError::Missing {
            setting: Setting::Host,
            ..
        }
    ));
    assert_eq!(
        err.to_string(),
        format!(
            "missing setting `host`: set `host` in {}, the PFLEASE_HOST environment \
             variable, or --host",
            path.display()
        )
    );
}

#[test]
fn missing_host_without_any_config_path_points_at_config_flag() {
    let err = load_with(&Options::default(), &[]).expect_err("no host");
    assert_eq!(
        err.to_string(),
        "missing setting `host`: set `host` in the config file (--config), the \
         PFLEASE_HOST environment variable, or --host"
    );
}

#[test]
fn empty_host_counts_as_missing() {
    let (_dir, path) = file("host = \"\"\n");
    let err = load_with(&with_config(&path), &[]).expect_err("empty host");
    assert!(matches!(
        err,
        ConfigError::Missing {
            setting: Setting::Host,
            ..
        }
    ));
}

#[test]
fn empty_env_vars_count_as_unset() {
    let (_dir, path) = file(FULL_FILE);
    let settings = load_with(
        &with_config(&path),
        &[
            ("PFLEASE_HOST", ""),
            ("PFLEASE_FIREWALL", ""),
            ("PFLEASE_PORT", ""),
        ],
    )
    .expect("loads");
    assert_eq!(settings.host, "file.example");
    assert_eq!(settings.port, Some(port(2201)));
}

#[test]
fn empty_user_leaves_it_to_ssh() {
    let settings = load_with(
        &Options::default(),
        &[("PFLEASE_HOST", "h"), ("PFLEASE_USER", "")],
    )
    .expect("loads");
    assert_eq!(settings.user, None);
    let (_dir, path) = file("host = \"h\"\nuser = \"\"\n");
    let settings = load_with(&with_config(&path), &[]).expect("loads");
    assert_eq!(settings.user, None);
}

#[test]
fn missing_default_file_is_not_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().to_str().expect("utf-8 temp path");
    let settings = load_with(
        &Options::default(),
        &[("HOME", home), ("PFLEASE_HOST", "h")],
    )
    .expect("loads");
    assert_eq!(settings.host, "h");
}

#[test]
fn missing_explicit_config_file_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("absent.toml");
    let err = load_with(&with_config(&path), &[("PFLEASE_HOST", "h")]).expect_err("absent");
    let ConfigError::Read { path: got, source } = &err else {
        panic!("expected a read error, got {err:?}");
    };
    assert_eq!(got, &path);
    assert_eq!(source.kind(), io::ErrorKind::NotFound);
    assert!(
        err.to_string()
            .starts_with(&format!("cannot read config file {}: ", path.display())),
        "{err}"
    );
}

#[test]
fn unknown_key_such_as_a_password_is_rejected_without_its_value() {
    let (_dir, path) = file("host = \"h\"\npassword = \"hunter2\"\n");
    let err = load_with(&with_config(&path), &[]).expect_err("unknown key");
    let ConfigError::Parse { location, .. } = &err else {
        panic!("expected a parse error, got {err:?}");
    };
    assert_eq!(*location, Some(Location { line: 2, column: 1 }));
    assert_eq!(
        err.to_string(),
        format!(
            "invalid config file {}: unknown field `password`, expected one of `host`, \
             `user`, `port`, `identity_file`, `lease_file`, `interfaces`, `firewalls` (line 2, column 1)",
            path.display()
        )
    );
}

#[test]
fn unknown_key_in_a_named_firewall_is_rejected() {
    let (_dir, path) = file("[firewalls.lab]\nhost = \"h\"\napi_key = \"k\"\n");
    let err = load_with(&with_config(&path), &[]).expect_err("unknown key");
    assert!(err.to_string().contains("unknown field `api_key`"), "{err}");
}

#[test]
fn invalid_toml_names_the_file() {
    let (_dir, path) = file("host = \n");
    let err = load_with(&with_config(&path), &[]).expect_err("bad toml");
    let ConfigError::Parse { location, .. } = &err else {
        panic!("expected a parse error, got {err:?}");
    };
    assert_eq!(*location, Some(Location { line: 1, column: 8 }));
    assert!(
        err.to_string()
            .starts_with(&format!("invalid config file {}: ", path.display())),
        "{err}"
    );
}

#[test]
fn zero_port_in_the_file_is_rejected() {
    let (_dir, path) = file("host = \"h\"\nport = 0\n");
    let err = load_with(&with_config(&path), &[]).expect_err("port 0");
    assert!(matches!(err, ConfigError::Parse { .. }), "{err:?}");
}

#[test]
fn invalid_env_port_names_the_variable_and_the_value() {
    let err = load_with(
        &Options::default(),
        &[("PFLEASE_HOST", "h"), ("PFLEASE_PORT", "ssh")],
    )
    .expect_err("bad port");
    assert_eq!(
        err.to_string(),
        "invalid port: PFLEASE_PORT `ssh`: invalid digit found in string"
    );
}

#[test]
fn host_that_ssh_would_read_as_an_option_is_rejected() {
    let err = load_with(&Options::default(), &[("PFLEASE_HOST", "-oProxyCommand=x")])
        .expect_err("option-like host");
    assert_eq!(
        err.to_string(),
        "invalid host: `-oProxyCommand=x` starts with `-`"
    );
}

#[test]
fn user_that_ssh_would_read_as_an_option_is_rejected() {
    let err = load_with(
        &Options::default(),
        &[("PFLEASE_HOST", "h"), ("PFLEASE_USER", "-v")],
    )
    .expect_err("option-like user");
    assert_eq!(err.to_string(), "invalid user: `-v` starts with `-`");
}

#[test]
fn empty_flags_leave_the_file_and_env_settings() {
    let (_dir, path) = file(FULL_FILE);
    let options = Options {
        config: Some(path),
        firewall: Some(String::new()),
        flags: Layer {
            host: Some(String::new()),
            user: Some(String::new()),
            port: None,
            identity_file: Some(PathBuf::new()),
            lease_file: Some(String::new()),
            interfaces: None,
        },
    };
    let settings = load_with(&options, &[("PFLEASE_USER", "env-user")]).expect("loads");
    assert_eq!(
        settings,
        Settings {
            host: "file.example".into(),
            user: Some("env-user".into()),
            port: Some(port(2201)),
            identity_file: Some("/keys/file".into()),
            lease_file: "/file/dhcpd.leases".into(),
            interfaces: Subnets::default(),
        }
    );
}

#[test]
fn empty_file_paths_fall_back_to_the_defaults() {
    let (_dir, path) = file("host = \"h\"\nidentity_file = \"\"\nlease_file = \"\"\n");
    let settings = load_with(&with_config(&path), &[]).expect("loads");
    assert_eq!(settings.identity_file, None);
    assert_eq!(settings.lease_file, DEFAULT_LEASE_FILE);
}

#[test]
fn empty_values_in_a_named_firewall_leave_the_top_level_ones() {
    let (_dir, path) = file(
        "host = \"top.example\"\nlease_file = \"/top\"\n\n\
         [firewalls.lab]\nhost = \"\"\nlease_file = \"\"\n",
    );
    let options = Options {
        firewall: Some("lab".into()),
        ..with_config(&path)
    };
    let settings = load_with(&options, &[]).expect("loads");
    assert_eq!(settings.host, "top.example");
    assert_eq!(settings.lease_file, "/top");
}

#[test]
fn type_error_on_a_known_key_shows_its_value() {
    let (_dir, path) = file("host = \"h\"\nport = \"ssh\"\n");
    let err = load_with(&with_config(&path), &[]).expect_err("wrong type");
    assert_eq!(
        err.to_string(),
        format!(
            "invalid config file {}: invalid type: string \"ssh\", expected a nonzero u16 \
             (line 2, column 8)",
            path.display()
        )
    );
}

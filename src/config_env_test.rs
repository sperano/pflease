//! Config tests about the environment: where the default file is found,
//! and `PFLEASE_*` values that are not UTF-8.

use super::Options;
use super::tests::load_with;
#[cfg(unix)]
use super::{ConfigError, Setting, Settings, load};

#[test]
fn default_path_is_under_xdg_config_home() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config_dir = dir.path().join("pflease");
    std::fs::create_dir(&config_dir).expect("mkdir");
    std::fs::write(config_dir.join("config.toml"), "host = \"xdg.example\"\n").expect("write");
    let xdg = dir.path().to_str().expect("utf-8 temp path");
    let settings = load_with(
        &Options::default(),
        &[("XDG_CONFIG_HOME", xdg), ("HOME", "/nonexistent")],
    )
    .expect("loads");
    assert_eq!(settings.host, "xdg.example");
}

#[test]
fn default_path_falls_back_to_home_dot_config() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config_dir = dir.path().join(".config").join("pflease");
    std::fs::create_dir_all(&config_dir).expect("mkdir");
    std::fs::write(config_dir.join("config.toml"), "host = \"home.example\"\n").expect("write");
    let home = dir.path().to_str().expect("utf-8 temp path");
    let settings = load_with(&Options::default(), &[("HOME", home)]).expect("loads");
    assert_eq!(settings.host, "home.example");
}

#[test]
fn relative_xdg_config_home_is_ignored() {
    let err = load_with(
        &Options::default(),
        &[("XDG_CONFIG_HOME", "relative"), ("HOME", "/home/someone")],
    )
    .expect_err("no host");
    assert_eq!(
        err.to_string(),
        "missing setting `host`: set `host` in /home/someone/.config/pflease/config.toml, \
         the PFLEASE_HOST environment variable, or --host"
    );
}

#[cfg(unix)]
fn load_with_raw(name: &str, bytes: &'static [u8]) -> Result<Settings, ConfigError> {
    use std::os::unix::ffi::OsStrExt;
    let raw = std::ffi::OsStr::from_bytes(bytes).to_owned();
    let name = name.to_owned();
    load(&Options::default(), &move |var| match var {
        "PFLEASE_HOST" if name != var => Some("h".into()),
        _ if name == var => Some(raw.clone()),
        _ => None,
    })
}

/// Bytes that are not UTF-8.
#[cfg(unix)]
const NOT_UTF8: &[u8] = b"fw\xff";

#[cfg(unix)]
#[test]
fn non_utf8_env_values_are_rejected_by_name() {
    for setting in [
        Setting::Host,
        Setting::User,
        Setting::Port,
        Setting::LeaseFile,
        Setting::Firewall,
    ] {
        let err = load_with_raw(setting.env(), NOT_UTF8).expect_err("not UTF-8");
        assert_eq!(
            err.to_string(),
            format!(
                "invalid {}: {} is not valid UTF-8",
                setting.key(),
                setting.env()
            )
        );
    }
}

#[cfg(unix)]
#[test]
fn non_utf8_identity_file_is_kept_as_a_path() {
    use std::os::unix::ffi::OsStrExt;
    let settings = load_with_raw("PFLEASE_IDENTITY_FILE", NOT_UTF8).expect("loads");
    assert_eq!(
        settings
            .identity_file
            .as_deref()
            .map(|path| path.as_os_str().as_bytes()),
        Some(NOT_UTF8)
    );
}

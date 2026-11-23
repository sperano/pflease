use std::num::NonZeroU16;

use clap::{CommandFactory, Parser};
use pflease::config::{Layer, Options};

use super::{Cli, Command};

/// Port passed with `--port` in the mapping test.
const TEST_PORT: u16 = 2222;

#[test]
fn cli_definition_is_valid() {
    Cli::command().debug_assert();
}

#[test]
fn parses_list_in_local_time_by_default() {
    let cli = Cli::try_parse_from(["pflease", "list"]).expect("list parses");
    assert!(matches!(cli.command, Command::List { utc: false }));
}

#[test]
fn parses_list_utc() {
    let cli = Cli::try_parse_from(["pflease", "list", "--utc"]).expect("list --utc parses");
    assert!(matches!(cli.command, Command::List { utc: true }));
}

#[test]
fn config_flags_default_to_unset() {
    let cli = Cli::try_parse_from(["pflease", "list"]).expect("list parses");
    assert_eq!(cli.config.options(), Options::default());
}

#[test]
fn config_flags_map_to_options_after_the_subcommand() {
    let cli = Cli::try_parse_from([
        "pflease",
        "list",
        "--config",
        "/etc/pflease.toml",
        "--firewall",
        "lab",
        "--host",
        "fw.example",
        "--user",
        "admin",
        "--port",
        "2222",
        "--identity-file",
        "/keys/fw",
        "--lease-file",
        "/tmp/dhcpd.leases",
    ])
    .expect("flags parse");
    assert_eq!(
        cli.config.options(),
        Options {
            config: Some("/etc/pflease.toml".into()),
            firewall: Some("lab".into()),
            flags: Layer {
                host: Some("fw.example".into()),
                user: Some("admin".into()),
                port: NonZeroU16::new(TEST_PORT),
                identity_file: Some("/keys/fw".into()),
                lease_file: Some("/tmp/dhcpd.leases".into()),
            },
        }
    );
}

#[test]
fn config_flags_parse_before_the_subcommand() {
    let cli = Cli::try_parse_from(["pflease", "--host", "fw.example", "list"])
        .expect("global flag parses");
    assert_eq!(cli.config.host.as_deref(), Some("fw.example"));
}

#[test]
fn zero_port_flag_is_rejected() {
    let err = Cli::try_parse_from(["pflease", "list", "--port", "0"]).expect_err("port 0");
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
}

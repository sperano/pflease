use clap::{CommandFactory, Parser};

use super::{Cli, Command};

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

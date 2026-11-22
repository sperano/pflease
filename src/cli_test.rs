use clap::{CommandFactory, Parser};

use super::{Cli, Command};

#[test]
fn cli_definition_is_valid() {
    Cli::command().debug_assert();
}

#[test]
fn parses_list() {
    let cli = Cli::try_parse_from(["pflease", "list"]).expect("list parses");
    assert!(matches!(cli.command, Command::List));
}

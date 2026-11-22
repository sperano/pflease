//! `pflease`: list and filter pfSense DHCP leases in the terminal.

mod cli;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    match cli.command {
        cli::Command::List => println!("lease listing is not implemented yet"),
    }
}

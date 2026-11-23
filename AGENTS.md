# Repository agent guidance

Canonical, tool-neutral guidance for coding agents. Tool-specific files
(`CLAUDE.md`) import this file instead of copying it.

## Overview

`pflease` is a Rust command-line tool that reads the DHCP leases of a pfSense
firewall and shows them in the terminal.

Goals, in order:

1. List every lease (IP, MAC, hostname, start/end, state, interface).
2. Filter and sort them (by hostname, MAC, IP range, state, interface).
3. Promote a lease to a static mapping on the firewall.

Fetch leases over SSH from the single configured pfSense firewall. The
verified deployment runs pfSense Plus 26.03 with ISC DHCP, so the current
source is `/var/dhcpd/var/db/dhcpd.leases`. Parse ISC lease blocks with the
last block for each IP taking precedence. Derive interface names from the
configured IPv4 subnets under `interfaces` in `/conf/config.xml`; keep that
configuration in memory and never log it because it contains secrets.

Shell out to OpenSSH so the user's SSH config, agent and known hosts apply.
Current access uses the key-authenticated `admin` account; keep the SSH host
and user in runtime configuration rather than in source.
Keep transport and parsing separate behind `LeaseSource`; when this firewall
migrates to Kea, replace the parser and lease-file path without changing the
display or filter code. The REST API package is not installed, and ISC DHCP
is deprecated. Decide the write transport for static mappings separately
before implementing `promote`; SSH read access does not imply direct edits
to `config.xml`. Recorded, anonymized source fixtures are in
`tests/fixtures/`.

## Commands

```bash
cargo build
cargo run -- --help
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

CI (`.github/workflows/ci.yml`) runs the last three; green means all three
pass.

## Conventions

- Edition 2024, stable toolchain; `unsafe` is forbidden and clippy's
  `pedantic` group is on (see `[lints]` in `Cargo.toml`).
- Named constants, not magic numbers.
- Files stay under about 500 lines and functions under about 50; compose
  small functions.
- Unit tests live in a sibling file, never inline: `src/foo.rs` declares
  `#[cfg(test)] #[path = "foo_test.rs"] mod tests;` and the tests go in
  `src/foo_test.rs`.
- Tests never reach a real firewall: parse recorded fixtures and use a fake
  lease source.
- Errors carry context up to `main`, which prints them and exits non-zero;
  no `unwrap`/`expect` outside tests.
- Never commit credentials, firewall hostnames or real lease data.

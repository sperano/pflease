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

Leases are read over SSH from pfSense's ISC lease database
(`/var/dhcpd/var/db/dhcpd.leases`), by shelling out to `ssh` so the user's
`~/.ssh/config`, agent and `known_hosts` apply and no crypto crate is
needed. The REST API package is not installed on the target firewall and is
in maintenance mode, and the firewall still runs ISC `dhcpd`; keep the
source behind the `LeaseSource` trait so a Kea reader can replace it. The
write path for static mappings is decided separately, before `promote`.

Settings come from `~/.config/pflease/config.toml` (or `--config`), then
`PFLEASE_*` environment variables, then flags; see `src/config.rs`. No
setting holds a secret: SSH keys stay with `ssh`.

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

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

How leases are fetched is not decided yet. Candidates: the pfSense REST API
package, or SSH reading the lease database (ISC `dhcpd.leases`, or Kea's on
newer pfSense releases). Settle it in a PR before building on it, and keep
the source behind a trait so the display and filter code does not depend on
it.

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

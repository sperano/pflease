# pflease

List and filter pfSense DHCP leases from the terminal.

Early development: `pflease list` renders leases as a table (`--utc` for
UTC times), but no lease source is implemented yet, so it exits with
`no lease source configured`.

```bash
cargo run -- list
```

## License

GPL-3.0-or-later; see [LICENSE](LICENSE).

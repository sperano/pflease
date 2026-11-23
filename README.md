# pflease

List and filter pfSense DHCP leases from the terminal.

Early development: `pflease list` renders leases as a table (`--utc` for
UTC times), but no lease source is implemented yet, so it exits with
`no lease source configured`.

`list` filters with `--hostname` (substring), `--mac` (address or prefix),
`--ip` (CIDR, `a-b` range or address), `--state` and `--interface`.
Different flags must all match; repeating a flag matches any of its values.

```bash
cargo run -- list
cargo run -- list --ip 10.0.0.0/24 --state active --state free
```

## License

GPL-3.0-or-later; see [LICENSE](LICENSE).

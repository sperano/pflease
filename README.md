# pflease

List and filter pfSense DHCP leases from the terminal.

Early development: `pflease list` renders leases as a table (`--utc` for
UTC times) and reads its configuration, but no lease source is implemented
yet. It exits with ``missing setting `host` `` until a `host` is configured
(see below), then with `no lease source configured`.

`list` filters with `--hostname` (substring), `--mac` (address or prefix),
`--ip` (CIDR, `a-b` range or address), `--state` and `--interface`.
Different flags must all match; repeating a flag matches any of its values.
`--sort ip|mac|hostname|start|end|state|interface` (default `ip`) orders the
table, `--reverse` flips it; missing values always sort last and ties
follow ascending IP.

```bash
cargo run -- list
cargo run -- list --ip 10.0.0.0/24 --state active --state free
cargo run -- list --sort hostname --reverse
```

## Configuration

`pflease` reads the firewall's lease database over SSH, using your `ssh`
(its config, agent and `known_hosts`). It reads
`~/.config/pflease/config.toml` (`$XDG_CONFIG_HOME/pflease/config.toml` when
that is set to an absolute path), or the file given with `--config`:

```toml
# SSH destination: hostname, address or ~/.ssh/config alias. Required.
host = "firewall.example.net"
# Optional; unset leaves each to ssh.
user = "leases"
port = 22
identity_file = "/home/you/.ssh/id_ed25519_pfsense"
# Optional; defaults to pfSense's ISC lease database.
lease_file = "/var/dhcpd/var/db/dhcpd.leases"

# Named firewalls, selected with --firewall <name> or PFLEASE_FIREWALL.
# Their settings override the top-level ones.
[firewalls.lab]
host = "192.0.2.1"
```

Each setting can also be set with an environment variable or a flag. The
last one wins: config file (top level, then the selected firewall), then
environment, then flags.

| File key        | Environment variable    | Flag              |
| --------------- | ----------------------- | ----------------- |
| `host`          | `PFLEASE_HOST`          | `--host`          |
| `user`          | `PFLEASE_USER`          | `--user`          |
| `port`          | `PFLEASE_PORT`          | `--port`          |
| `identity_file` | `PFLEASE_IDENTITY_FILE` | `--identity-file` |
| `lease_file`    | `PFLEASE_LEASE_FILE`    | `--lease-file`    |
| (firewall name) | `PFLEASE_FIREWALL`      | `--firewall`      |

An empty value, in the file, the environment or a flag, counts as unset. No
setting holds a secret: the SSH key stays with `ssh`, and unknown keys such
as `password` are rejected without echoing their value.

## License

GPL-3.0-or-later; see [LICENSE](LICENSE).

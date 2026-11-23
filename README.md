# pflease

List and filter pfSense DHCP leases from the terminal.

`pflease list` reads the firewall's DHCP lease database over SSH and
renders it as a table, times in local time (`--utc` for UTC).

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

## Setup

On the firewall (pfSense with the ISC DHCP backend, not Kea):

1. Enable SSH: System > Advanced > Admin Access > Secure Shell, with public
   key authentication.
2. Give a user an authorized key and a login that runs commands passed by
   `ssh`: in System > User Manager, the user's "Authorized SSH Keys" and the
   "User - System: Shell account access" privilege. `pflease` runs `cat`
   on the lease file, `/var/dhcpd/var/db/dhcpd.leases`, which that user
   must be able to read.

On the workstation, check that `ssh` reaches it without prompting
(`pflease` runs `ssh` in batch mode, so a passphrase must come from the
agent and the host key must already be in `known_hosts`):

```bash
ssh leases@firewall.example.net cat /var/dhcpd/var/db/dhcpd.leases | head
```

Then write the config file below with at least `host`, and run
`pflease list`.

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

# Optional: the subnet each interface serves (CIDR, a-b range or address).
# The lease database records no interface, so a lease's interface is the
# narrowest of these holding its address; without a match it shows as -.
[interfaces]
lan = "192.0.2.0/24"
iot = "198.51.100.0/24"

# Named firewalls, selected with --firewall <name> or PFLEASE_FIREWALL.
# Their settings override the top-level ones; an [interfaces] table
# replaces the top-level one as a whole.
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

`[interfaces]` is set only in the file.

An empty value, in the file, the environment or a flag, counts as unset. No
setting holds a secret: the SSH key stays with `ssh`, and unknown keys such
as `password` are rejected without echoing their value.

## License

GPL-3.0-or-later; see [LICENSE](LICENSE).

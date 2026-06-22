## What this CIDR calculator does

Paste a CIDR block in `ADDRESS/PREFIX` notation and this tool computes everything you need to plan or document a subnet — entirely in your browser, so the address you enter never leaves your machine.

For an **IPv4** block it returns:

- **Network address** — the base address with the host bits cleared. A non-aligned host is normalized to its network, so `192.168.1.130/26` becomes `192.168.1.128`.
- **Broadcast address** — the address with every host bit set.
- **Netmask** and **wildcard mask** — e.g. `/24` gives `255.255.255.0` and `0.0.0.255`.
- **Usable host range** — the first and last assignable host. A `/31` is treated as an RFC 3021 point-to-point link (both addresses usable) and a `/32` as a single host.
- **Total addresses** and **usable hosts**.
- **Scope** — whether the block is private (RFC 1918 `10/8`, `172.16/12`, `192.168/16`, loopback, link-local, or CGNAT `100.64/10`) or public.

For an **IPv6** block it returns the network address, prefix length, the first and last address in the block, and the total address count — exact even for a `/0`, which spans 2^128 addresses.

## Examples

- `192.168.1.0/24` gives network `192.168.1.0`, broadcast `192.168.1.255`, 254 usable hosts.
- `10.0.0.0/30` gives 4 addresses, 2 usable hosts (`10.0.0.1`-`10.0.0.2`).
- `2001:db8::/64` gives 18,446,744,073,709,551,616 addresses.

Choose **text** for an aligned report or **json** for a machine-readable object you can pipe into other tooling.

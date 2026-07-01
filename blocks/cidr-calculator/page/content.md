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

## FAQ

<details>
<summary>What if I enter a host address instead of the network base?</summary>

The tool normalizes it for you: the host bits are cleared, so `192.168.1.130/26`
is reported as network `192.168.1.128/26`. That makes it an easy way to answer
"which subnet does this IP belong to?" — paste the host with its prefix and read
off the network, broadcast, and host range.

</details>

<details>
<summary>How are /31 and /32 blocks counted?</summary>

A `/31` follows RFC 3021: it's a point-to-point link, so **both** addresses are
usable hosts (no network/broadcast reservation). A `/32` is a single host with
exactly 1 usable address. For `/30` and larger blocks the usual rule applies —
total addresses minus the network and broadcast addresses.

</details>

<details>
<summary>Does it support IPv6, including huge prefixes like /0?</summary>

Yes. Any prefix from `/0` to `/128` works. For IPv6 you get the network
address, the first and last address in the block, and an exact total address
count — even a `/0`, whose 2^128 addresses overflow ordinary integers, is
reported exactly. Broadcast and netmask are IPv4 concepts and only appear for
IPv4 blocks.

</details>

<details>
<summary>Which ranges are flagged as private?</summary>

For IPv4, the scope check covers RFC 1918 (`10.0.0.0/8`, `172.16.0.0/12`,
`192.168.0.0/16`), loopback (`127.0.0.0/8`), link-local (`169.254.0.0/16`), and
carrier-grade NAT (`100.64.0.0/10`). Anything else is reported as public.

</details>

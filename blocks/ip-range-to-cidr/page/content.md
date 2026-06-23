## What this tool does

**IP Range to CIDR** takes an arbitrary inclusive start–end IP range and converts
it into the *minimal* set of CIDR blocks that exactly covers it — no extra
addresses, no missing ones. It is the inverse of expanding a CIDR into a list of
addresses, and works for both IPv4 and IPv6.

Paste a range like `10.0.0.5-10.0.0.20` and you get the fewest aligned CIDR
blocks that cover it:

```
10.0.0.5/32
10.0.0.6/31
10.0.0.8/29
10.0.0.16/30
10.0.0.20/32
```

A range that happens to be aligned collapses to a single block —
`192.168.1.0-192.168.1.255` becomes `192.168.1.0/24`.

## Input formats

- **IPv4 range** — `10.0.0.5-10.0.0.20`, `192.168.1.0-192.168.1.255`.
- **IPv4 shorthand** — a bare final octet on the right is expanded, so
  `192.168.1.10-20` means `192.168.1.10-192.168.1.20`.
- **IPv6 range** — `2001:db8::-2001:db8::ffff`, `2001:db8::1-2001:db8::5`.
- **Single address** — an address with no `-` (e.g. `10.0.0.5`) returns a single
  host route, `/32` for IPv4 or `/128` for IPv6.

Both endpoints must be the same family, and the start must not be greater than
the end.

## Output

- **List** (default) — the minimal CIDR blocks, one per line, ready to paste into
  a firewall rule, route table, or allow-list.
- **Count** — just how many CIDR blocks the range needs.

## Why CIDR aggregation matters

Firewalls, routers, and ACLs are configured with CIDR blocks, not raw start–end
ranges. Converting a range to the fewest CIDR blocks keeps rule tables small and
fast, and avoids accidentally allowing or blocking addresses outside the range.
The algorithm always returns the *minimal* cover, so you never get more blocks
than strictly necessary.

## Private by design

Everything runs locally in your browser via WebAssembly. Your IP ranges are
never uploaded to a server — there is no network call, no logging, and no
sign-up.

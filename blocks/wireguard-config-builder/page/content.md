## About this tool

This builder turns the values you already have — your interface private key, the peer's public
key, addresses, endpoint and AllowedIPs — into a complete, ready-to-save WireGuard config file
(`wg0.conf`). Unlike a plain template, **every field is validated** before the file is produced:

- Each key must base64-decode to exactly **32 bytes** (the Curve25519 key length), so a
  truncated or mistyped key is rejected with a clear message instead of silently producing a
  config that never connects.
- Every **Address**, **AllowedIPs** and **DNS** entry is parsed as a real IPv4/IPv6 address, and
  CIDR prefixes are range-checked (0–32 for IPv4, 0–128 for IPv6).
- The **Endpoint** is checked as `host:port` (IPv6 literals must be bracketed, `[2001:db8::1]:51820`),
  and every **ListenPort**, **MTU** and **PersistentKeepalive** value is range-checked.

Everything runs locally in your browser via WebAssembly — your keys are never uploaded.

## This tool does NOT generate keys

By design, it validates and assembles keys you paste; it does not create them. Key generation
needs a cryptographically secure random source and is non-deterministic, which does not fit a
recompute-on-input page. Generate your keypair first with the official `wg` tools:

```
wg genkey | tee privatekey | wg pubkey > publickey     # interface keypair
wg genpsk > preshared                                  # optional preshared key
```

Then paste the interface **PrivateKey**, the peer's **PublicKey**, and (optionally) the
**PresharedKey** into the fields above.

## Worked example

For a full-tunnel client — private key `AAAA…AAA=`, address `10.0.0.2/32`, DNS `1.1.1.1, 8.8.8.8`,
peer public key `MTIz…MDEy=`, `AllowedIPs = 0.0.0.0/0, ::/0`, endpoint `vpn.example.com:51820`,
keepalive `25` — the tool emits:

```
[Interface]
PrivateKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
Address = 10.0.0.2/32
DNS = 1.1.1.1, 8.8.8.8

[Peer]
PublicKey = MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = vpn.example.com:51820
PersistentKeepalive = 25
```

Save that as `wg0.conf` and bring it up with `wg-quick up ./wg0.conf`.

## Full tunnel vs. split tunnel

**AllowedIPs** on the peer decides what traffic goes through the tunnel:

- `0.0.0.0/0, ::/0` — a **full tunnel**: route *all* IPv4 and IPv6 traffic through the peer (a
  typical VPN client).
- `10.0.0.0/24` (or any specific subnet) — a **split tunnel**: only that subnet is routed through
  WireGuard; everything else uses your normal connection.

## Limits and edge cases

- **Single interface, single peer.** The form assembles one `[Interface]` and one `[Peer]`. Hub
  configs with many peers are outside this tool's scope — assemble each peer block separately.
- **No key generation and no QR export.** Paste keys from `wg genkey`/`wg pubkey`; QR/image output
  has no text-page render mode here.
- Fields left blank are simply omitted, so a roaming client with no `ListenPort` or a server with
  no `Endpoint` both produce a clean file.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions -->

<details>
<summary>Does this tool generate my WireGuard keys?</summary>

No. It validates and assembles keys you paste — it never creates them. Generate a keypair with
`wg genkey | wg pubkey` (and `wg genpsk` for an optional preshared key), then paste the values in.
Key generation needs a secure random source and can't be a deterministic page function.

</details>

<details>
<summary>What should I put in AllowedIPs?</summary>

Use `0.0.0.0/0, ::/0` to route **all** traffic through the peer (a full VPN tunnel), or a specific
subnet such as `10.0.0.0/24` for a **split tunnel** that only routes that network. Each entry is a
CIDR and is validated; you can list several separated by commas.

</details>

<details>
<summary>Why is my key rejected as "not 32 bytes"?</summary>

WireGuard keys are Curve25519 keys: exactly 32 raw bytes, which base64-encode to a 44-character
string ending in `=`. If your input decodes to a different length it was truncated, mistyped, or
isn't a WireGuard key. Re-copy the full output of `wg genkey`/`wg pubkey`.

</details>

<details>
<summary>How do I write an IPv6 endpoint?</summary>

Bracket the IPv6 literal and put the port after the bracket: `[2001:db8::1]:51820`. A bare
`2001:db8::1:51820` is ambiguous (the last group looks like the port) and is rejected. Domain names
and IPv4 endpoints use the plain `host:port` form, e.g. `vpn.example.com:51820`.

</details>

<details>
<summary>Do I need ListenPort, MTU or PersistentKeepalive?</summary>

All three are optional. A roaming client can omit **ListenPort** (WireGuard picks a random port);
**MTU** defaults to 1420 and only needs changing if you see fragmentation; set
**PersistentKeepalive** to `25` when this side is behind NAT so the tunnel stays open. Blank fields
are left out of the file entirely.

</details>

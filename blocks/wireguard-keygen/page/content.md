## About this tool

This generator creates the keys a **WireGuard** tunnel needs, in exactly the form the
`wg` command-line tools print them:

- a **private key** — 32 Curve25519 bytes as 44 base64 characters, the same value
  `wg genkey` produces, including the clamping (`b[0] &= 248; b[31] &= 127; b[31] |= 64`)
  that `wg genkey` applies before printing;
- the matching **public key**, the Curve25519 base-point multiplication of that private
  key — identical to piping the private key through `wg pubkey`;
- an optional **preshared key**, 32 fresh random bytes like `wg genpsk`, which adds a
  symmetric layer on top of the Curve25519 handshake.

Alongside the keys it renders an annotated `wg0.conf` snippet: an `[Interface]` block to
keep on this device, a `[Peer]` block for the remote side, and a commented peer fragment
you can hand to whoever runs the other end so they can reach you back.

Everything runs locally in WebAssembly using your browser's cryptographically secure
random generator. No key is uploaded, logged, or stored — reloading the page loses it.

## Worked example

With **Number of key pairs** at 1, a preshared key enabled, the tunnel address
`10.0.0.2/32` and the endpoint `vpn.example.com:51820`, the output looks like this
(your keys will differ — every run draws fresh randomness):

```
PrivateKey   = qNXZ0Q0DwQqZ0kO2kV4vXQ0rXK3PjE5Q7bA6y2fT9Vc=
PublicKey    = Hs7bQ8k2mR1yF4tJvZ9pL0nC3xW6eA5uD8gK1oS7iT4=
PresharedKey = 9jL2vB5nQ8xR1cM4tY7wZ0aD3fG6hK9pS2uX5eH8bN0=

# Sample wg0.conf — keep the private key on this device only.
[Interface]
PrivateKey = qNXZ0Q0DwQqZ0kO2kV4vXQ0rXK3PjE5Q7bA6y2fT9Vc=
Address = 10.0.0.2/32
# ListenPort = 51820   # servers only; clients pick a random port

[Peer]
# Paste the REMOTE side's public key here.
PublicKey = <remote peer public key>
PresharedKey = 9jL2vB5nQ8xR1cM4tY7wZ0aD3fG6hK9pS2uX5eH8bN0=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = vpn.example.com:51820
PersistentKeepalive = 25

# Hand the other side this block so it can reach you:
# [Peer]
# PublicKey = Hs7bQ8k2mR1yF4tJvZ9pL0nC3xW6eA5uD8gK1oS7iT4=
# PresharedKey = 9jL2vB5nQ8xR1cM4tY7wZ0aD3fG6hK9pS2uX5eH8bN0=
# AllowedIPs = 10.0.0.2/32
```

Switching **Output** to *JSON (one object per key pair)* returns the same material as
`{ "key_pairs": [ { "index": 1, "private_key": …, "public_key": …, "preshared_key": …,
"config": … } ] }`, which is what you want when scripting a batch of peers.

## Options

- **Number of key pairs** — 1 to 25 independent pairs in one run. Generate one pair per
  device; a bulk run is for provisioning several peers at once, not for reusing one key.
- **Also generate a preshared key** — on by default. The preshared key must be written
  into *both* peers' `[Peer]` sections, identically, or the handshake fails.
- **Output** — *Keys + annotated wg0.conf snippet* (default) for reading and copying;
  *JSON* for scripts; *wg0.conf snippet only* when you just want a file to save.
- **Tunnel address (CIDR)** — the `[Interface] Address` for this device. A comma list and
  IPv6 both work, e.g. `10.0.0.2/32, fd00::2/128`. Clients usually take a `/32`; a server
  interface takes the whole subnet, e.g. `10.0.0.1/24`.
- **Server endpoint (host:port)** — the `[Peer] Endpoint` the client dials. Write IPv6 as
  `[fd00::1]:51820`. Leave it blank for a server's own config, which listens rather than
  dials, and the line is omitted.

## Limits

- Up to **25 key pairs** per run, so the result stays a copy-pasteable page of text.
- The snippet is a starting point, not a validated multi-peer configuration: the remote
  peer's public key is left as a placeholder, and `AllowedIPs`, DNS, MTU and
  `ListenPort` are not checked here. The WireGuard config builder tool on this site
  validates and assembles a complete `wg0.conf` from values you already have.
- This tool only **generates** keys. It will not derive a public key from a private key
  you paste in, and it cannot reproduce a previous key — there is no seed input.
- No QR code output. Use a QR generator on the finished config if you are importing it
  into a phone.
- Nothing is written to disk for you. Copy or download the output and store it with
  restrictive permissions (`chmod 600`) yourself.

## FAQ

<details>
<summary>Is it safe to generate WireGuard keys in a browser?</summary>

The cryptography here is sound: the key comes from `crypto.getRandomValues`, the
browser's CSPRNG, and the derivation runs in WebAssembly on your machine with no network
request anywhere in the key path. That said, the strongest practice — and the one the
WireGuard documentation recommends — is to generate a private key **on the machine that
will use it** and never move it. Use this page for convenience, lab work, and peers you
are provisioning anyway; for a key protecting something valuable, run `wg genkey` on the
target host instead.

</details>

<details>
<summary>Does this produce the same keys as `wg genkey` and `wg pubkey`?</summary>

Yes, in the sense that matters: the private key is 32 random bytes clamped with the same
`b[0] &= 248; b[31] &= 127; b[31] |= 64` mask `wg genkey` applies, base64-encoded to the
same 44 characters, and the public key is the Curve25519 base-point multiplication of it,
byte-for-byte what `wg pubkey` would print for that private key. You can verify it: paste
the private key into `wg pubkey` and you will get the public key shown here. The keys
themselves differ every run, because they are random.

</details>

<details>
<summary>Do I need the preshared key?</summary>

It is optional. WireGuard's Curve25519 handshake is secure without one; the preshared key
adds a symmetric secret mixed into the handshake, which is mainly valuable as
post-quantum hedging — an attacker recording traffic today cannot decrypt it later with a
quantum computer unless they also stole the preshared key. The cost is key management:
the same value has to be present in the `[Peer]` section on **both** ends, and a
mismatch breaks the tunnel silently apart from a failed handshake. Turn it off if you do
not want to distribute a third secret.

</details>

<details>
<summary>Which key goes where in the config?</summary>

A device's **own private key** goes in its `[Interface] PrivateKey`. The **other side's
public key** goes in the local `[Peer] PublicKey`. So the two config files cross over:
your public key appears in the server's file, and the server's public key appears in
yours. The commented block at the end of the snippet is exactly the fragment to send the
other side — it contains only your public key (and the preshared key), never your
private key.

</details>

<details>
<summary>How should I store the private key once I have it?</summary>

Put the config at `/etc/wireguard/wg0.conf` and restrict it before anything else reads
it — `chmod 600` on the file, owned by root. Use a separate key pair for every device so
that losing one phone does not mean rotating the whole network, and rotate keys on a
schedule you can actually keep. Keep `AllowedIPs` as narrow as the routing allows;
`0.0.0.0/0, ::/0` in the example is a full-tunnel setup, which is often not what a
site-to-site link wants.

</details>

<details>
<summary>I lost my public key but still have the private one. Can I recover it?</summary>

Yes, but not with this tool — the public key is derived from the private key, so
`echo "<private key>" | wg pubkey` recomputes it on any machine with the WireGuard tools
installed. This generator deliberately has no private-key input: accepting one would mean
pasting a live secret into a form field, which is the habit these tools should not teach.

</details>

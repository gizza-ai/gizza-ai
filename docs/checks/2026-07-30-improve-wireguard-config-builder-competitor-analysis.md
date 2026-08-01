# wireguard-config-builder — competitor analysis (2026-07-30)

Scope: build **and validate** a complete WireGuard `[Interface]` + `[Peer]` config from
values the user *enters* (keys, addresses, endpoints, AllowedIPs). Pure compute — no key
generation, no network. All paraphrased; no competitor copy/branding reproduced.

## Competitors scanned (top real tools)

1. **longqt-sea wg-generator** (browser, GitHub Pages) — fill server address + client
   count, emits server + client `.conf` files; generates keypairs client-side.
2. **jmrp.io "MikroTik WireGuard Config Generator"** — visual builder producing
   router-flavoured configs (dual-stack, firewall/NAT snippets) plus a client config;
   100% in-browser, keys never leave the device.
3. **nixpoin.com WireGuard Config Generator** — generates complete server + client
   configs instantly in-browser, including Curve25519 keypair generation and
   multi-client support.

(Also seen: Stellaxon, RapidToolSet, IPv64.net, UpVPN — same feature envelope.)

## Table-stakes params → decision

A WireGuard config has exactly two section types; the fields below are the union every
competitor exposes. Each ends in our descriptor (in-model) or the out-of-model list.

`[Interface]`
- **PrivateKey** — in-model (`private_key`, required; validated: base64 of exactly 32 bytes).
- **Address** — in-model (`address`, required; comma list of CIDRs / bare IPs, each validated).
- **ListenPort** — in-model (`listen_port`, optional int 1–65535).
- **DNS** — in-model (`dns`, optional; comma list of IPs, each validated).
- **MTU** — in-model (`mtu`, optional int; sanity range 576–9000, typical 1420).

`[Peer]`
- **PublicKey** — in-model (`peer_public_key`, required; base64 32-byte validated).
- **PresharedKey** — in-model (`preshared_key`, optional; base64 32-byte validated).
- **AllowedIPs** — in-model (`allowed_ips`, required; comma list of CIDRs, each validated).
- **Endpoint** — in-model (`endpoint`, optional; `host:port` / `[v6]:port` validated).
- **PersistentKeepalive** — in-model (`persistent_keepalive`, optional int 0–65535, typical 25).

Output format:
- **`.conf` text** — in-model (default `format = conf`).
- **structured JSON** — in-model (`format = json`) — a nicety for tooling; our validation
  differentiator surfaces the parsed fields.

## UX control patterns matched

- Number fields for port / MTU / keepalive (descriptor `Param::integer`).
- Preset **chips** (`[[example]]`) for the two AllowedIPs archetypes competitors default to:
  full-tunnel `0.0.0.0/0, ::/0` and a split-tunnel LAN example, plus a filled sample config.
- `format` enum rendered as a `<select>`.

## Out-of-model (listed, NOT built)

- **Keypair generation** (Curve25519). Non-deterministic (needs a CSPRNG) → does not fit the
  page's recompute-on-input model, and this tool's remit is *entered* keys. Users pair it with
  `wg genkey`. Noted in copy.
- **Multi-peer / multi-client fan-out & QR export.** The page form is a single fixed set of
  fields (no dynamic peer list) and image/QR output has no text-page render mode. Single
  interface + single peer is the clean in-model scope. Noted in copy/FAQ.
- **Router-vendor snippets** (MikroTik/OPNsense firewall+NAT). Vendor-specific, out of the
  generic-config remit.

## Validation (our differentiator vs. plain generators)

- Keys must base64-decode to exactly 32 bytes (WireGuard Curve25519 key length) — rejects
  truncated/mistyped keys with an actionable message.
- Every Address / AllowedIPs / DNS entry parsed with `std::net` (v4/v6, prefix range checked).
- Endpoint host:port shape + port range checked; IPv6 endpoint must use `[..]:port`.
- Port / MTU / keepalive range-checked.

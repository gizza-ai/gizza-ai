# wireguard-keygen — competitor analysis (2026-08-16)

Scan run BEFORE implementing, per `/improve-tool` Phase 2. One WebSearch
(`WireGuard key generator online generate private public preshared key`) plus a
skim of the top real competitor tools. No competitor copy, wording, or branding
was reused — only the *capability set* was compared.

## Competitors reviewed

| # | Tool | What it does | Notable |
|---|------|--------------|---------|
| 1 | UpVPN — WireGuard Key Generator (`upvpn.app/wireguard-key-generator/`) | Generates public/private key pairs in-browser | **Number of key pairs** control (bulk), optional preshared key, **JSON output**, download button; punts full configs to a separate config tool |
| 2 | RandomKeygen — WireGuard Key Generator Guide (`randomkeygen.com/wireguard-key`) | Guide-first page around `wg genkey` / `wg pubkey` / `wg genpsk` | Ships **sample `wg0.conf` for both server and client**, and a best-practices section (unique key per device, never share the private key, `chmod 600`, key rotation, restrictive `AllowedIPs`, PSK for sensitive links) |
| 3 | wg-keygen-notrust (`github.com/jcarrano/wg-keygen-notrust`) | Client-side, trustless browser keygen | Emits **two artifacts**: a config file for the user *and* a `[Peer]` fragment to hand the admin; parameterised URLs let an admin pre-fill the form; hard "keys never leave your computer" guarantee |

Also noted from the search: a strong, repeated security position that WireGuard
private keys should be generated **locally**, never by a remote server — every
serious competitor states this explicitly.

## Table stakes → decisions

| Capability | Competitors | Decision |
|---|---|---|
| Curve25519 private + public key, base64 (44 chars) | 1, 2, 3 | **Build** — the core output. Private key is **clamped exactly like `wg genkey`** (`b[0] &= 248; b[31] &= 127; b[31] \|= 64`) so output is byte-indistinguishable from the real tool and round-trips through `wg pubkey`. |
| Preshared key (`wg genpsk`) | 1, 2 | **Build** — `preshared_key` boolean, on by default, one fresh 32-byte PSK per pair. |
| Bulk generation ("number of key pairs") | 1 | **Build** — `pairs` integer, 1–25, default 1. |
| JSON output | 1 | **Build** — `format = "json"`, one object per pair. |
| Sample `wg0.conf` snippet | 2, 3 | **Build** — `format = "conf"`, and the snippet is also appended in `text` mode. Backed by `address` + `endpoint` params so the snippet is real, not a placeholder blob. |
| Peer fragment for the admin | 3 | **Build** — the conf snippet carries both the `[Interface]` block (your private key) and the matching `[Peer]` block to hand over, so the two-artifact split is covered without a second output mode. |
| Download the result | 1 | **Already shipped** — the generator gives `format = "text"` pages a Download link for free. |
| Pre-fillable/shareable URL | 3 | **Already shipped** — the tool page reads every param from `?query=` deep links. |
| Runs locally, keys never uploaded | 1, 2, 3 | **Build (copy)** — stated in the hero, the content body, and the FAQ; true here (wasm in-page, CLI local, no network capability in the descriptor). |
| Security best practices copy | 2 | **Build (copy)** — FAQ covers `chmod 600`, one key pair per device, rotation, when a PSK is worth it. |

## Deliberately not built (out of model / already covered)

- **Full interactive config builder** (server + client + DNS + MTU + AllowedIPs validation) — already a separate shipped tool, `blocks/wireguard-config-builder`; this tool cross-references it rather than duplicating it.
- **QR code for mobile import** — covered by the existing `qr-code-generator` block; adding image output would cost this tool its text page.
- **Email-the-fragment-to-your-admin integration** (wg-keygen-notrust) — requires network/mail I/O, out of model for a pure block.
- **IPFS / content-addressed hosting** as a trust mechanism — a deployment property of the host site, not a tool feature.
- **Deriving a public key from a pasted private key** (`wg pubkey` on existing input) — that is a *parse*, not a generate; it belongs to the existing key-inspection family, and mixing it in would make a non-deterministic tool conditionally deterministic.

## Not a duplicate

- `blocks/wireguard-config-builder` explicitly states in its own module docs that it **does not generate keys** ("that needs a CSPRNG and is non-deterministic") — it validates pasted keys. This tool fills exactly that stated gap.
- `blocks/keypair-generator` generates generic X25519/Ed25519 pairs as **PKCS#8 / SPKI PEM** with unclamped raw bytes — not WireGuard-shaped output, no PSK, no config snippet, no `wg genkey` clamping.

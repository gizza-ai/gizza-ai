# nostr-key-encoder — competitor analysis (2026-08-01)

**Tool:** Converts Nostr identifiers between raw hex and NIP-19 bech32
(`npub`/`nsec`/`note`/`nprofile`/`nevent`, plus decode-only `naddr`/`nrelay`).
Pure Rust, no deps — plain Bech32 (BIP 173) checksum, no 90-char cap.

## Scan

One WebSearch ("nostr NIP-19 key converter tool npub nsec hex nprofile nevent
encode decode"). Reviewed the canonical spec and the real reference tools /
libraries (paraphrased only — no copy, branding, or trademarks reused):

- **NIP-19 spec** (nostr-protocol/nips #19): defines the bare types (`npub`,
  `nsec`, `note`) as bech32 over a 32-byte value and the TLV types (`nprofile`,
  `nevent`, `naddr`, `nrelay`) carrying a special value plus optional relay /
  author / kind / identifier records. Notes that Nostr uses plain Bech32, drops
  BIP 173's length limit, and suggests a ~5000-char soft cap.
- **Online converter (nostr-tools.com)** — a browser tool that converts between
  hex and every NIP-19 form (npub/nsec/note/nprofile/nevent/naddr) and decodes
  any identifier back to hex plus its structured fields.
- **nostr-tools (nbd-wtf, JS `nip19.ts`)** — the reference library: per-type
  `*Encode`/`decode` functions, TLV assembly, checksum handling. The de-facto
  behavior other clients match.
- **nec (straumer)** — a small CLI that encodes/decodes bech32 Nostr entities per
  NIP-19; direction inferred from the input.
- **nostr_nip19 (Dart)** — a library covering encode/decode of the same set of
  identifiers, showing this is table-stakes across ecosystems.

## Table-stakes → in-model / out-of-model

| Capability | Decision |
|---|---|
| Encode 32-byte hex → npub / nsec / note | **in-model** — `type` enum + `require_32` |
| Decode any bech32 identifier → hex | **in-model** — `bech32_decode` + bare-type path |
| Auto-detect direction from the input | **in-model** — `mode="auto"` (prefix sniff → decode, else encode) |
| Force encode / decode | **in-model** — `mode` enum (auto \| encode \| decode) |
| nprofile: pubkey + relay hints (TLV) | **in-model** — `type="nprofile"`, `relays` param |
| nevent: id + relays + author + kind (TLV) | **in-model** — `type="nevent"` + `relays`/`author`/`kind` |
| Multiple relays, flexibly separated | **in-model** — comma / space / newline split |
| Decode TLV → labeled report of every field | **in-model** — `render_tlv` (pubkey/id/relay/author/kind) |
| Decode-only naddr / nrelay | **in-model** — reported with their fields (special as UTF-8) |
| Plain Bech32 (BIP 173), no 90-char cap, 5000 soft cap | **in-model** — matches the spec exactly |
| Clear validation errors (length, checksum, mixed case, bad char) | **in-model** — descriptive `Err` per case |
| Generate a fresh keypair (random nsec/npub) | **out-of-model** — key *generation* needs a secure RNG + is a distinct tool concern; this tool converts existing identifiers, it doesn't mint keys |
| Derive npub from an nsec (secp256k1 pubkey) | **out-of-model** — requires elliptic-curve point math (a signing-grade dep), beyond a bech32 codec; a separate crypto tool's job |
| Sign / verify events | **out-of-model** — full Nostr event crypto, not an identifier converter |
| naddr / nrelay *encoding* | **considered, rejected** — decode covers the common read path; encoding naddr needs a d-tag/identifier + kind + author bundle that would bloat the schema for a rare authoring case; the five common types cover the vast majority of use |
| Fetch profile metadata from relays | **out-of-model** — needs network + a running relay connection; gizza tools are offline/browser-local |

Every table-stake landed in the descriptor except the explicitly listed
out-of-model rows (key generation, pubkey derivation, signing, relay fetch) and
the one considered-rejected row (naddr/nrelay encoding).

## UX / controls

Reference tools offer a single box with auto-detected direction and separate
fields for TLV extras. Ours mirrors that with: `[[example]]` preset chips (decode
an npub, encode hex→npub, build an nprofile with relays), a `mode` `<select>`
(auto/encode/decode) and a `type` `<select>` with friendly labels
(`npub — public key`, etc.), multiline `input` and `relays` fields (so pasted
newlines and multi-relay lists survive), and dedicated `author`/`kind` fields for
nevent. Output is either the bare hex (bare types) or a labeled multi-line report
(TLV types). Errors name what was expected — exact byte length, checksum failure,
mixed case, or an unknown prefix — instead of a bare "invalid input".

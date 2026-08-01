## About this tool

**Nostr Key Encoder** converts Nostr identifiers between their raw hex form and
the human-readable NIP-19 bech32 form used across Nostr clients. Paste a 64-char
hex public key, private key, or event id to **encode** it into `npub`, `nsec`, or
`note`; or paste any NIP-19 string (`npub1…`, `nsec1…`, `note1…`, `nprofile1…`,
`nevent1…`) to **decode** it back to hex. It also builds and reads the richer TLV
entities — `nprofile` (a pubkey plus relay hints) and `nevent` (an event id plus
optional relays, author, and kind).

Leave the direction on **Auto-detect** and the tool decodes anything that starts
with a known NIP-19 prefix and encodes everything else. Everything runs locally
in your browser as WebAssembly — your keys and ids are never uploaded, which
matters because an `nsec` is a secret.

### Worked example

Encoding the public-key hex
`7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e` as an `npub`
gives:

```
npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg
```

Decoding that same `npub` returns the original hex. Decoding a TLV entity returns
a labeled report instead of a bare key, for example an `nprofile`:

```
type: nprofile
pubkey: 3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d
relay: wss://r.x.com
relay: wss://djbas.sadkb.com
```

### Options

- **Direction** — *Auto-detect* (decode a NIP-19 string, otherwise encode),
  *Encode* (force hex → bech32), or *Decode* (force bech32 → hex/report).
- **Encode as** — the target NIP-19 type when encoding hex: `npub` (public key),
  `nsec` (private key), `note` (event id), `nprofile` (pubkey + relays), or
  `nevent` (event id + relays/author/kind). Ignored on decode — the type is read
  from the string's prefix.
- **Relays** — for `nprofile`/`nevent`, optional relay URLs where the entity can
  be found. Separate several with commas, spaces, or newlines.
- **Author pubkey hex** — for `nevent`, an optional 64-char hex pubkey of the
  event's author, stored as a TLV hint.
- **Event kind** — for `nevent`, an optional Nostr event kind (0–4294967295, e.g.
  `1` for a text note). Leave it at `-1` to omit the kind.

### Limits

- A bare `npub`/`nsec`/`note` (and the special value inside `nprofile`/`nevent`)
  must be exactly **32 bytes / 64 hex characters**; a leading `0x` and any
  whitespace in hex input are ignored.
- Nostr uses plain **Bech32 (BIP 173)** checksums, not bech32m, and — unlike BIP
  173 — imposes **no 90-character cap**; NIP-19 suggests a 5000-character soft
  limit instead, which this tool enforces.
- `naddr` and `nrelay` are supported for **decoding** only (they are reported with
  their fields); encoding is limited to the five common types above.
- This tool converts identifiers — it does **not** generate keys, derive a public
  key from a private key, or sign events.

## FAQ

<details>
<summary>What is the difference between npub, nsec, and note?</summary>

They are three NIP-19 bech32 encodings of a 32-byte value with different prefixes:
`npub` wraps a **public key**, `nsec` wraps a **private key** (keep it secret),
and `note` wraps an **event id**. The underlying hex is the same length for all
three — only the human-readable prefix and checksum differ.

</details>

<details>
<summary>What are nprofile and nevent for?</summary>

They are **TLV** (type-length-value) entities that bundle extra hints alongside
the key or id. An `nprofile` carries a pubkey plus optional relay URLs where that
profile can be found; an `nevent` carries an event id plus optional relays, the
author's pubkey, and the event kind. Clients use the relay hints to locate the
entity without a global index.

</details>

<details>
<summary>Does the tool detect the direction automatically?</summary>

Yes. On **Auto-detect** (the default), any input beginning with a known NIP-19
prefix (`npub1`, `nsec1`, `note1`, `nprofile1`, `nevent1`, `naddr1`, `nrelay1`) is
decoded, and anything else is treated as hex and encoded to the selected type. Set
the direction to **Encode** or **Decode** to force it.

</details>

<details>
<summary>Is my private key (nsec) safe to paste here?</summary>

The conversion runs entirely in your browser as WebAssembly, so nothing you type —
including an `nsec` — is sent to a server. That said, always be cautious with any
private key: only use a device and browser you trust.

</details>

<details>
<summary>Why does decoding say "invalid Bech32 checksum"?</summary>

Every NIP-19 string ends in a 6-character checksum, so a single mistyped or
dropped character makes the whole string fail validation. Copy the identifier in
full — including its prefix — and try again. Nostr uses the plain Bech32 (BIP 173)
checksum, so bech32m strings (like Bitcoin `bc1p…` taproot addresses) won't
validate.

</details>

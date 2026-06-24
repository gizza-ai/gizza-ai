# base58-codec — competitor analysis (2026-06-22)

Tool: `blocks/base58-codec` — encode text/bytes to Base58 and decode back.
Surfaces: chat skill (block.wasm), CLI (`gizza tool base58-codec`), standalone
page (`/tools/base58-codec/`). Pure Rust, runs on all backends incl. the chat
service worker.

## Top competitors surveyed

1. **Browserling** (browserling.com/tools/base58-encode, /base58-decode) — split
   encode and decode pages, no-ads, text I/O only, single (Bitcoin) alphabet.
2. **emn178 Online Tools** (emn178.github.io/online-tools/base58/) — encode +
   decode; lets you choose the *output/input encoding* (UTF-8, UTF-16, Hex,
   Base64) so binary data round-trips. Single alphabet.
3. **dCode Base58 Converter** (dcode.fr/base-58-cipher) — encode/decode, and
   notably exposes a **custom/selectable alphabet** (Bitcoin, Ripple, Flickr,
   and arbitrary). Strong on alphabet choice.
4. **CodeBeautify Base58 Decode** (codebeautify.org/base58-decode) — text + URL +
   **file upload** input, copy/download buttons, ad-supported.
5. **LDDGO Base58** (lddgo.net/en/convert/base58) — string and **file** encode/
   decode, multi-language UI.

## Capability diff vs our tool

| Capability | Competitors | gizza base58-codec | Status |
| --- | --- | --- | --- |
| Encode + decode in one tool | most (Browserling splits) | yes (single `mode` switch) | matched / better |
| Bitcoin alphabet (default) | all | yes | matched |
| Ripple (XRP) alphabet | dCode | yes (`variant=ripple`) | matched |
| Flickr alphabet | dCode | yes (`variant=flickr`) | matched |
| Hex byte I/O for binary data | emn178, dCode | yes (`format=hex`, `0x`/spaces tolerated) | matched |
| Leading-zero-byte preservation | implicit in all correct impls | yes (explicit, tested) | matched |
| Runs locally / private / offline | Browserling, emn178 | yes (WASM, no server) | matched |
| Deep-linkable (query-param prefill) | rare | yes (`?input=…&variant=…`) | better |
| Available via chat + CLI (not just web) | none | yes (3 surfaces) | better |
| Custom/arbitrary alphabet | dCode only | no | gap (see below) |
| File upload input | CodeBeautify, LDDGO | no | out of model (page) |
| Base58**Check** (version byte + checksum) | dCode (separate mode) | no | distinct tool |
| Base64/UTF-16 I/O encodings | emn178 | text + hex only | minor gap |

## Gaps closed this pass

The scaffold shipped a single `input` param. Brought the tool to parity with the
best free competitors by adding, in the descriptor/core/web/page + manifest:

- **`mode`** encode/decode in one tool (vs Browserling's split pages).
- **`variant`** = bitcoin / ripple / flickr — matches dCode's named-alphabet set,
  the widest in the field.
- **`format`** = text / hex byte I/O so binary data (key hashes, tx ids) round-trips,
  matching emn178/dCode; `0x` prefix and whitespace tolerated on hex input.
- Explicit, tested **leading-zero preservation** (each `0x00` → `1`), the property
  Bitcoin addresses rely on.
- Page deep-linking + UTF-8 error guidance ("switch to hex") in the copy.

## Out-of-model / deferred (not built — per skill rules)

- **File upload input.** The page input is a single text field; a binary file
  upload for base58 would need an `AssetKind` page input + accept type. Several
  competitors offer it; deferred (same limitation noted for other codec tools).
- **Custom/arbitrary alphabet.** dCode lets you type any 58-char alphabet. Our
  `variant` is a fixed enum (the three real-world alphabets). A free-text 58-char
  alphabet param is feasible but low-value vs. risk of user error; left out.
- **Base58Check** (version byte + double-SHA256 checksum, e.g. for BTC addresses)
  is a *distinct* tool, not a variant of plain Base58 — a candidate for its own
  backlog entry rather than scope-creep here.
- **Extra I/O encodings** (Base64, UTF-16 in/out) — text+hex covers the practical
  cases; not added.

## Verification (all surfaces, 2026-06-22)

- `cargo test` (block): schema drift-guard passes.
- `cargo test -p gizza-ai-base58-codec-core`: 14/14 unit tests pass (Bitcoin text +
  hex vectors, leading zeros, ripple/flickr round-trips, error paths).
- `wafer build`: block.wasm validates (300.5 KiB).
- `wasm-pack build … web`: page wasm builds.
- CLI: `gizza tool base58-codec input="Hello World!"` → `2NEpo7TZRRrLZSi2U`;
  decode and ripple/hex variants verified.
- Playwright `tool-page-base58-codec.spec.ts`: 6/6 pass (encode, decode, hex,
  leading-zero, ripple, query-param deep-link).

## Sources

- [Browserling Base58 Encode](https://www.browserling.com/tools/base58-encode)
- [Browserling Base58 Decode](https://www.browserling.com/tools/base58-decode)
- [emn178 Base58 Encode](https://emn178.github.io/online-tools/base58/encode/)
- [dCode Base58 Converter](https://www.dcode.fr/base-58-cipher)
- [CodeBeautify Base58 Decode](https://codebeautify.org/base58-decode)
- [LDDGO Base58](https://www.lddgo.net/en/convert/base58)

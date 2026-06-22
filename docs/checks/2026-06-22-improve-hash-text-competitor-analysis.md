# hash-text — competitor analysis & improvement check (2026-06-22)

Tool: **hash-text** — compute a cryptographic hash of pasted text with a
selectable algorithm. Surfaces: chat (LLM API) + CLI + standalone page.

## What it does

Single text input + an `algorithm` selector (MD5, SHA-1, SHA-224, SHA-256
[default], SHA-384, SHA-512, SHA3-256, SHA3-512, BLAKE2b-512, BLAKE2s-256,
BLAKE3) + input decoding (`text`/`hex`/`base64`) + output rendering
(`hex`/`base64`, optional uppercase). Pure-Rust (RustCrypto family + `blake3`),
so it runs on every backend including the chat Service Worker. Verified against
published "abc" test vectors for every algorithm.

## Relationship to existing gizza blocks (dup check)

- `sha256-hash` — text in, **SHA-256 only**, no algorithm choice. hash-text is a
  strict superset on the algorithm axis (it also offers MD5/SHA-1/SHA-2 family/
  SHA-3/BLAKE2/BLAKE3) while keeping the same hex/base64/uppercase/decoding
  options. Not a dup — distinct value is the algorithm picker.
- `file-hash` — **file/bytes** input, emits MD5+SHA-1+SHA-256+SHA-512+CRC-32 all
  at once; no SHA-3/BLAKE, no single-algorithm text mode. Different input shape
  (file vs text) and different output shape (all-at-once vs one selectable
  digest). Not a dup.
- `argon2-hash`, `bcrypt-hash` — password KDFs (salted, slow, tuneable cost),
  a different category entirely.
- `hash-identifier` — guesses the algorithm of an existing digest; inverse task.

Conclusion: hash-text is a distinct general-purpose text-hashing tool. Kept.

## Top competitors surveyed

General-purpose online "hash generator / hash calculator" tools that take pasted
text and a selectable algorithm. Surveyed for capability/feature coverage only —
no copy, branding, or trademarks were taken from any of them.

1. Multi-algorithm online hash generators (the common "hash calculator" form
   factor) — typically offer MD5, SHA-1, SHA-256, SHA-512 and sometimes SHA-3.
2. Developer-utility "all hashes at once" pages — compute several digests in one
   shot from one input.
3. CRC/checksum-oriented calculators.
4. Privacy-positioned "in-browser, nothing uploaded" hashers.
5. CLI-style reference hashers (the `*sum` family: md5sum/sha256sum/b2sum/b3sum).

## Capability gap analysis (in-model = pure-Rust / browser-local)

| Capability | Competitors | hash-text | Status |
|---|---|---|---|
| MD5 / SHA-1 / SHA-256 / SHA-512 | common | yes | covered |
| SHA-224 / SHA-384 | sometimes | yes | covered (closes a common gap) |
| SHA-3 (256/512) | sometimes | yes | covered |
| BLAKE2b / BLAKE2s | rare | yes | covered (differentiator) |
| BLAKE3 | rare | yes | covered (differentiator) |
| Hex output (+ uppercase) | common | yes | covered |
| Base64 output | sometimes | yes | covered |
| Hash raw bytes via hex/base64 input | rare | yes | covered (differentiator) |
| In-browser, no upload (privacy) | some | yes | covered |
| Lenient algorithm aliases (SHA-256 = sha256 = sha-256) | n/a | yes | UX polish |
| Deep-link via query params (`?text=…&algorithm=…`) | rare | yes | covered |
| File input | some | no — use `file-hash` | by design (separate tool) |
| HMAC (keyed hash) | some | no | out of current scope; candidate for a
  dedicated `hmac` tool (keyed, distinct param shape) rather than overloading
  this one |
| CRC-32/CRC-16 checksums | some | no | covered by `file-hash` (CRC-32); a text
  CRC could be a minor future add |
| Streaming / very large file hashing | some | n/a (text tool) | out of scope |

## Improvements applied this pass

- Broadened the algorithm menu well beyond the typical competitor (added the
  full SHA-2 family incl. SHA-224/384, SHA-3 256/512, BLAKE2b/BLAKE2s, BLAKE3) —
  this is the core differentiator versus the single-algorithm `sha256-hash` and
  most online generators.
- Lenient algorithm parsing: case-insensitive and `-`/`_`/no-separator
  insensitive (`SHA3_256`, `sha3-256`, `sha3256` all resolve), plus `blake2b`/
  `blake2s` shorthands — reduces friction without expanding the displayed menu.
- Kept the proven `sha256-hash` ergonomics (hex/base64 input decoding,
  hex/base64 output, uppercase) so users migrating lose nothing.
- SEO/page copy explicitly flags MD5/SHA-1 as broken-for-security (checksum-only)
  so users pick an appropriate algorithm — an honesty/UX edge over generators
  that present all algorithms as equivalent.
- Cross-links to `file-hash` for whole-file hashing (and the multi-digest case).

## Out-of-model / deliberately not built

- **HMAC / keyed hashing** — distinct parameter shape (requires a key + key
  encoding); better as its own tool than overloading this one. Noted as a future
  candidate, not built.
- **File input** — already served by `file-hash`; not duplicated here.

## Verification (all surfaces, 2026-06-22)

- `cargo test --workspace` in `blocks/hash-text/` — 20 core tests (published
  "abc" vectors for every algorithm + encoding/format/error paths) + the
  descriptor↔authored-schema drift guard all pass.
- `wafer build` — chat `block.wasm` validates and instantiates (374.5 KiB);
  `sha3`/`blake2`/`blake3` all instantiate in the wasm32-wasip1 wafer runtime.
- `wasm-pack build …/web` — page wasm built.
- CLI: `gizza tool hash-text …` returns correct digests for sha256/md5/blake3/
  sha3-512, base64 output, uppercase, hex input, and a clear error for an
  invalid algorithm.
- Page: `tests/tool-page-hash-text.spec.ts` — 7 Playwright tests pass (default
  hash, md5, blake3, base64 output, uppercase checkbox, hex-input encoding,
  query-param deep-link).

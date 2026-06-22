# sha1-hash — competitor analysis & improvement snapshot (2026-06-22)

Tool: **SHA-1 Hash Generator** (`blocks/sha1-hash`). Pure-compute, runs on all
surfaces (chat Service Worker, CLI, standalone page). Computes the SHA-1 (160-bit)
digest of text with selectable input decoding and output format.

## Surfaces verified

- **Chat block** — `wafer build` validates `target/block.wasm` (318.7 KiB, RustCrypto
  `sha1` instantiates cleanly in wasm32-wasip1). Drift-guard schema test passes.
- **CLI** — `gizza tool sha1-hash text=abc` → `a9993e364706816aba3e25717850c26c9cd0d89d`
  (matches the FIPS 180 known vector); uppercase, base64, hex-input, and the bad-encoding
  error path all verified.
- **Page** — 5 Playwright tests pass (default hex, base64 output, uppercase checkbox,
  hex-input decode, and `?text=&output_format=` deep-link prefill).

## Top competitors surveyed

1. **xorbin SHA-1 hash generator** — single text box → hex digest. No options.
2. **emn178 online-tools (SHA-1)** — text/file input, hex output, live-as-you-type.
3. **MD5/SHA hashing sites (e.g. md5calc, browserling)** — text → hex, often bundled
   with several algorithms.
4. **CyberChef "SHA1" operation** — text/byte input, hex output, part of a recipe chain.
5. **RapidTables / DuplichChecker SHA-1 generators** — text → uppercase/lowercase hex.

## Capability diff

| Capability                         | Competitors (typical) | gizza sha1-hash |
|------------------------------------|-----------------------|-----------------|
| Hash text → hex                    | yes                   | yes             |
| Uppercase hex toggle               | some                  | yes             |
| Base64 digest output               | rare (CyberChef only) | yes             |
| Decode hex/base64 input first      | rare (CyberChef only) | yes             |
| Runs fully client-side / private   | varies (many POST)    | yes (wasm)      |
| Deep-link / query-param prefill    | no                    | yes             |
| Same engine across chat + CLI + web| no                    | yes             |
| Security warning (SHA-1 is broken) | rarely shown          | yes (copy)      |

## Gaps closed in this build

- **Input decoding** (`input_encoding=hex|base64`) so users can hash raw bytes / keys
  / ciphertext, not just UTF-8 — matches CyberChef's most useful extra, beats simple
  single-box competitors.
- **Output format** hex/base64 + **uppercase** toggle.
- **Explicit security warning** in the page copy and the chat skill description: SHA-1
  is cryptographically broken (SHAttered, 2017) and must not be used for security —
  pointing users to the existing `sha256-hash` tool. Most competitors omit this.
- **Cross-references** to `file-hash` (whole-file digests incl. MD5/SHA-256/SHA-512/CRC-32)
  and `sha256-hash` (secure alternative) in copy + descriptions.

## Out-of-model (not built — by design)

- **File upload on the page**: hashing an uploaded binary file is covered by the existing
  `file-hash` tool (`AssetKind::Any`); a single-algorithm SHA-1 file page would duplicate it.
- **HMAC-SHA1 / keyed digests**: a distinct keyed-MAC tool, out of scope for a plain digest tool.
- **Live-as-you-type streaming**: the page already recomputes on each input change, which
  is functionally equivalent for short text.

## Dedup note

Not a duplicate. The repo's own precedent is dedicated single-algorithm hash blocks
(`sha256-hash`, `keccak-hash`) living alongside the multi-algorithm `hash-text`; `sha1-hash`
is the SHA-1 sibling of `sha256-hash` and shares its core shape. `file-hash` hashes uploaded
files (different IO), `hash-text` is the multi-algorithm picker — neither makes a focused,
SEO-targeted SHA-1 text page redundant.

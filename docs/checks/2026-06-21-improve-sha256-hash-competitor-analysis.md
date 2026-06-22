# sha256-hash — competitor analysis (2026-06-21)

## Tool
SHA-256 (SHA-2) hash generator for **text**. Pure Rust (RustCrypto `sha2`),
runs on all backends (chat Service Worker, CLI, standalone page). Inputs:
`text` (required), `input_encoding` (text|hex|base64), `output_format`
(hex|base64), `uppercase` (bool). Output: the 256-bit digest as hex/base64.

Scope note: this is the **text** SHA-256 tool with its own SEO page (the CSV
rationale: SHA-256 is the single most-searched hash, "warranting its own page").
The existing `file-hash` block covers **file** hashing (MD5/SHA-1/SHA-256/
SHA-512/CRC-32, no page) — distinct IO shape, so not a duplicate.

## Surfaces verified
- **chat block**: `wafer build` validates `target/block.wasm` instantiates (322 KiB).
- **CLI**: `gizza tool sha256-hash text=abc` → known vector
  `ba7816bf…20015ad`; base64, uppercase, hex-input, and the bad-hex error path
  all confirmed.
- **page**: 5 Playwright tests pass — default hex, base64 output, uppercase
  checkbox, hex input-encoding, and `?text=&output_format=` query-param deep-link.
- **unit**: 10 core tests (known SHA-256 vectors incl. empty string + "abc",
  hex/base64 input round-trips, error cases) + 1 schema drift-guard.

## Top competitor tools surveyed
1. **xorbin / SHA-256 hash online** — single text box → hex digest. No options.
2. **passwordsgenerator.net SHA-256** — text → hex, uppercase toggle.
3. **CyberChef "SHA2" recipe** — text/bytes → hash, selectable bit-length and
   input format (raw/hex/base64), part of a recipe chain.
4. **emn178 online-tools sha256** — text/file → hex, supports hex/base64 input
   and live "hash as you type".
5. **md5calc.com / various** — text → hex/base64, uppercase, file upload.

## Gap diff (fit-to-model)
| Capability | Competitors | Ours | Status |
|---|---|---|---|
| Text → hex digest | all | yes | covered |
| Live recompute on input | emn178, most | yes (page recomputes per keystroke) | covered |
| Uppercase hex | passwordsgenerator, md5calc | yes (`uppercase`) | covered |
| Base64 output | md5calc, CyberChef | yes (`output_format=base64`) | covered |
| Hex/base64 **input** decoding | CyberChef, emn178 | yes (`input_encoding`) | covered |
| File hashing | emn178, md5calc | via separate `file-hash` block | covered elsewhere (scope) |
| Selectable SHA-2 bit length (224/384/512) | CyberChef | no | **out of scope** — separate tool per hash keeps each page focused; SHA-512/etc. covered by file-hash and would be their own pages |
| HMAC-SHA256 | CyberChef | no | out of scope — distinct tool (keyed MAC, not a plain digest) |
| Privacy / offline | varies | yes (WASM, nothing uploaded) | parity / advantage |

## Decisions
- Closed every in-model gap a single-purpose SHA-256 **text** page should have:
  hex + base64 output, uppercase, and hex/base64 input decoding.
- Did NOT add other SHA-2 lengths or HMAC — those are distinct tools, not
  copy/feature gaps for a SHA-256 page; bundling them would blur the SEO page.
- No competitor copy, branding, or trademarks were used.

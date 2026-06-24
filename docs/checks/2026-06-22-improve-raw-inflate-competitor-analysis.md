# raw-inflate — competitor analysis (2026-06-22)

**Tool:** `gizza-ai/raw-inflate` — decompress a file containing headerless **raw
DEFLATE** (RFC 1951) back to its original bytes. No gzip (RFC 1952) and no zlib
(RFC 1950) wrapper is expected — just the bare deflate bit stream. Inverse of the
existing `raw-deflate` block.

**Type:** pure (flate2 / miniz_oxide, wasm32-safe). File-input → file-output, so
**chat + CLI** surfaces only — **no standalone page** (binary file→file output is
the no-page file-input pattern, same as `raw-deflate` / `gunzip`).

## Surfaces verified (Phase 1)

| Surface | Status | Evidence |
|---|---|---|
| chat block.wasm | PASS | `wafer build` validated/instantiated the block (508.6 KiB). |
| CLI happy path | PASS | `gizza tool raw-inflate url=<host>/payload.bin` → "inflated payload.bin (59 bytes raw DEFLATE) → payload.bin.out (1350 bytes original, 22.9x expansion)"; output bytes compared **byte-for-byte equal** to the original 1350-byte payload. |
| CLI error path | PASS | feeding a **gzip-wrapped** stream → `not a valid raw DEFLATE stream: corrupt deflate stream` (rejected, not silently mis-decoded). |
| page | N/A | binary file→file output has no page render mode (documented limitation). |
| unit tests | PASS | 5 core tests (round-trip, highly-compressed, empty, gzip-rejected, garbage-rejected) + 1 drift-guard schema test. |

## Competitors surveyed (top 5)

1. **dCode — Deflate Compression/Decompression** (dcode.fr) — compress + decompress
   RFC 1951; decompressor outputs text / hex / dec / oct / bin / base64 / file; has a
   metadata parser and block-type (static/dynamic/stored Huffman) identifier; notes a
   raw deflate stream "has no universal marker".
2. **Webacus — ZLIB / INFLATE-RAW** (app.webacus.dev) — dedicated raw-inflate
   operation ("reverse of Deflate (raw)"); chainable with other zlib ops; copy/save/
   undo/redo UX.
3. **CodeBeautify — Zlib Decompress Online** — decompress zlib/deflate text or file
   to plain text in the browser.
4. **nayuki.io — Simple DEFLATE decompressor** — reference inflater (Java/Python/C++/
   TS), educational, exact RFC 1951 semantics.
5. **libdeflate / uzlib (ebiggers, pfalcon)** — native libraries doing raw DEFLATE /
   zlib / gzip decompress; whole-buffer, no streaming wrapper. Used as the
   correctness/behaviour reference for "raw vs wrapped".

## Gap diff + ranking (fit-to-model)

| Competitor capability | gizza raw-inflate | Verdict |
|---|---|---|
| Correct raw RFC 1951 inflate, exact byte fidelity | YES (flate2/miniz_oxide; byte-equal round-trip verified) | **At parity** — core capability covered. |
| Reject non-raw input (gzip/zlib wrapper, corrupt) with a clear message instead of garbage | YES — `not a valid raw DEFLATE stream: …`; gzip explicitly rejected | **Better than the lenient browser tools**, which can mis-handle wrong framing. |
| Large input handling | YES — 128 MiB cap | At parity / better than small browser textareas. |
| Output as text / hex / base64 / dec / oct / bin (dcode) | NO — returns the original **bytes** as a download (mime `application/octet-stream`) | **Out-of-model:** binary inflate output is a file download in gizza's envelope model; re-encoding the bytes as hex/base64 views belongs to separate encode tools (`base64`, `hex`, etc.), not this one. Listed, not built. |
| Paste input as text/hex/base64 (dcode/webacus) | NO — input is a file via `url`⊕`ref` | **Out-of-model:** this is a file-input/no-page tool; the chat/CLI source model is url/ref. A base64/hex-text input variant would be a different (page-shaped) tool. Listed, not built. |
| Compression level on the compress side | N/A — this tool only **inflates** (decompression has no level); the compress direction is the sibling `raw-deflate` block | Correctly out of scope. |
| Block-type / metadata inspector (dcode) | NO | Out-of-model nicety; a DEFLATE-stream **inspector** would be a distinct analysis tool, not an inflate tool. Listed, not built. |

**Conclusion:** no in-model capability, copy, UX or visual gap requires a code change.
The tool matches competitors on the core inflate capability with exact byte fidelity,
and is stricter (clean rejection of gzip/zlib-wrapped or corrupt input) than the
lenient browser decompressors. The remaining competitor features (alternate
output/input encodings, stream metadata inspection) are out-of-model for a
binary file→file inflate block and are intentionally left to dedicated
encode/inspect tools rather than bolted on here.

## Filename behaviour

Output strips a trailing `.deflate` / `.raw` / `.zz` suffix to recover the original
name (e.g. `data.txt.deflate` → `data.txt`); otherwise it appends `.out` so the
download never collides with the compressed input.

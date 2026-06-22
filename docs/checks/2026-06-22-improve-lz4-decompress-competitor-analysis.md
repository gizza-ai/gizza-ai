# lz4-decompress — competitor analysis & improvement snapshot (2026-06-22)

## Tool

`gizza-ai/lz4-decompress` — decompress a standard LZ4 **frame** stream (`.lz4`,
frame magic `0x184D2204`) back to its original bytes, returned as a downloadable
file. Pure-Rust `lz4_flex` (frame feature, no C liblz4), so it compiles to
wasm32 and runs on ALL backends including the chat Service Worker. It is the
exact inverse of the existing `lz4-compress` tool, and of any standard
`lz4 -d` / `unlz4`.

Surfaces: **chat + CLI**. No standalone page — this is the no-page file→file
pattern (a binary blob in, an arbitrary blob out), same as `gunzip`,
`lz4-compress`, `bzip2-compress`. There is no page render mode that fits an
arbitrary-bytes output.

## Phase 1 — surface verification (all green)

- **Block (chat):** `wafer build` compiled and **validated/instantiated**
  `target/block.wasm` (512 KiB). lz4_flex frame decoder instantiates clean in
  wasm32-wasip1 (no missing WASI imports).
- **CLI (over the wire):**
  - `lz4-compress url=…/octocat/Hello-World/README` → `README.lz4` (28 B), then
    `lz4-decompress url=…/README.lz4` → `Hello World!\n` (13 B) — **byte-exact**
    round-trip, `.lz4` suffix correctly stripped to `hello`.
  - Larger file: Rust `README.md` (3304 B) → `.lz4` (2055 B, ~38% smaller) →
    decompressed back to **3304 B that match the original byte-for-byte**
    (Python `a == b` → True).
- **Schema drift-guard:** `schema_json_matches_authored_chat_schema` unit test
  asserts the derived chat schema equals the authored `url`⊕`ref` `oneOf` JSON.
- **Core unit tests (6):** round-trip, repetitive data (>10× ratio),
  empty payload, full 0–255 binary cycle, rejects non-LZ4 / empty / 2-byte /
  gzip-magic input, and errors on a truncated frame.

## Competitors surveyed (top 5)

1. **`lz4` CLI / liblz4** (lz4.org, github.com/lz4/lz4) — the reference
   decompressor (`lz4 -d` / `unlz4`). Fully local, fastest, supports frame +
   legacy formats and block-API. Requires a local install + a terminal.
2. **ezyZip — Extract LZ4** (ezyzip.com) — browser-local extraction, "files
   never uploaded", privacy-first, drag-and-drop.
3. **jsontotable.org LZ4 Decompressor** (unLZ4) — free online, "ultra-fast and
   secure", paired with an LZ4 compressor.
4. **Snoka LZ4 Extractor** (lz4extractor.snoka.ca) — free online, extraction
   happens locally in the browser (not sent to a server).
5. **Beautify Code / Hexmos** (beautifycode.net, hexmos.com) — online
   decompress-resource utilities, no registration.

## Capability diff & gap ranking (fit-to-model)

| Capability | Competitors | gizza lz4-decompress | Verdict |
|---|---|---|---|
| Decompress standard LZ4 **frame** (`.lz4`, interoperable with `lz4 -d`) | all | yes (lz4_flex frame decoder, verified byte-exact) | **matched** |
| Frame magic validation / clear error on wrong type | lz4 CLI | yes — checks `04 22 4D 18` up front, rejects gzip/empty/garbage with a readable message | **matched / better** (most online tools just fail opaquely) |
| Runs with NO upload to a third-party server | ezyZip, Snoka | yes — runs in the chat Service Worker locally; CLI is fully local | **matched** |
| Decompression-bomb guard (output cap) | none documented | yes — 256 MiB output cap | **better** |
| Recover original filename | lz4 CLI (from `.lz4` name only — LZ4 frame has no FNAME field) | yes — strips `.lz4` suffix | **matched** (the LZ4 frame format, unlike gzip, stores no embedded filename, so suffix-strip is the maximum possible) |
| Available from an LLM/chat + scriptable CLI + as a tool ref in a pipeline | none | yes (3 ways) | **better** — competitors are web-UI-only |
| Legacy LZ4 frame format (pre-v1.5.0, magic `0x184C2102`) | lz4 CLI | no | **OUT OF MODEL**: `lz4_flex`'s frame decoder targets the modern frame format only; legacy is a compatibility wart effectively unused since 2015. Deliberately omitted, matching `lz4-compress` which only emits the modern frame. |
| Raw LZ4 **block** format (no frame header) | lz4 CLI (`--no-frame-crc` block API) | no | **OUT OF MODEL** for this tool: a raw block needs the caller to supply the original uncompressed size out-of-band (the block format stores no length), which doesn't fit a single-file-in tool. The frame format (what `lz4-compress` and `lz4` produce by default) is the universal interchange form. |

## Improvements applied

The build was authored against the competitor survey from the start (it mirrors
the already-improved `lz4-compress`), so no Phase-3/4 rework was needed:

- **Frame-magic preflight check** — validates `04 22 4D 18` and returns
  `"not an LZ4 frame (missing 04 22 4D 18 magic)"` so a mis-fed gzip/zip/garbage
  file gives an actionable error instead of an opaque decode failure (an edge
  most online decompressors handle poorly).
- **Decompression-bomb guard** — 256 MiB output cap via `take()`, mirroring
  `gunzip`, so a tiny malicious `.lz4` can't exhaust memory.
- **Honest skill copy** — describes the tool as the exact inverse of
  `lz4-compress` / `lz4 -d`, points `.tar.lz4` users to `extract-tar`, and makes
  no claim of legacy/block-format support that the engine can't deliver.

## Out-of-model (NOT built, by design)

- Legacy LZ4 frame format and raw block format (see table) — `lz4_flex`'s frame
  decoder covers the modern frame, which is the universal interchange format.
- A standalone page — arbitrary-bytes output has no page render mode (file→file
  no-page pattern, consistent with `gunzip` / `lz4-compress`).

No competitor copy, branding, or trademarks were used.

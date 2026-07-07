# archive-extractor — competitor analysis (2026-07-06)

New tool: `gizza-ai/archive-extractor`. A universal, auto-detecting archive
extractor. Detects the format from the leading magic bytes (no filename needed)
and unpacks it, repacking every extracted file into a single ZIP for download.

Surfaces: **chat + CLI, no standalone page** (a ZIP file output fits neither the
pure-text nor the ffmpeg media page shape — the established F3 no-page
file-input pattern shared by `extract-tar`, `unzip`, `gunzip`, `zip-inspect`,
`document-text-extract`, `pdf-table-extract`).

## Distinctness vs. existing gizza tools

gizza already ships single-format tools: `unzip` (zip → inline files),
`extract-tar` (tar/tar.gz → ZIP), `gunzip` (gz → bytes), `zip-inspect` (list
only), `identify-archive-format` (detect only, no extract), plus compress-side
tools and `lzma-decompress` / `lz4-decompress`. None of these auto-detect across
formats or extract bzip2 / xz / zstd content. `archive-extractor` is the
"drop any archive, we figure it out and unpack it" category — a distinct
product, not a dup of any single existing tool (the skiplisted `unzip-archive`
was zip-only = literally `unzip`). It also adds first-time extraction of bzip2,
xz, zstd, and lz4 payloads and the full `.tar.*` layered family.

## Competitors scanned (paraphrased; no copy/branding reproduced)

One WebSearch for "online universal archive extractor". Top real tools skimmed:

1. **Universal extractor A (formatfuse-style)** — ZIP/7Z/RAR/TAR/GZ/BZ2/XZ + ISO,
   CAB, CPIO, AR, LHA; in-browser, auto-detect, nothing uploaded to a server.
2. **Universal extractor B (convertico-style)** — ZIP/RAR/7Z/TAR/TAR.GZ/GZ/BZ2/XZ/CAB;
   auto-format detection, no install.
3. **Universal extractor C (tembrica-style)** — 70+ formats incl. ZIP/RAR/7z/TAR/
   GZ/BZ2/XZ/ZST/LZ4/ISO/CAB/ARJ/CPIO/LZH.
   (Also seen: ezyzip-style "no size limit, local only", openazip-style "wasm
   decoders in browser" — corroborate the same feature set.)

## Table-stakes → in-model / out-of-model

| Capability | Decision | Where |
|---|---|---|
| Auto-detect format from bytes (no manual pick) | in-model | `core::detect` (magic numbers) |
| Multi-format: zip, tar, gzip, bzip2, xz, zstd | in-model | all 6 decoders wired |
| lz4 (frame) — table-stake for 3/5 competitors, trivially feasible (`lz4_flex` already proven) | in-model (ADDED, beyond the 6 in the brief) | `Format::Lz4` |
| Layered `.tar.gz` / `.tar.bz2` / `.tar.xz` / `.tar.zst` / `.tar.lz4` | in-model | decompress → inner-tar peek |
| File listing (names + sizes + dirs) | in-model | `Extracted.entries`, echoed in `for_llm` |
| Download each contained file | in-model (as one ZIP bundle) | output ZIP envelope |
| Local / private (no server upload) | in-model | pure-Rust wasm runs client-side (chat SW / CLI) |
| Path-traversal / zip-bomb safety | in-model | `safe_path`, 10k-entry + 256 MiB caps |
| Zero-config UX (just drop the archive) | in-model | `Input::File` only, no params — matches every competitor |
| 7z, RAR, ISO, CAB, ARJ, LHA, cpio, ar | out-of-model | proprietary/heavy; RAR has no open decoder, 7z decode is heavy/risky in wasm. `identify-archive-format` still *detects* these. |
| Encrypted / password-protected archives | out-of-model | `zip` crate is deflate-only, no AES/ZipCrypto decrypt |
| Recursively unpack nested archives-in-archives | out-of-model | one layer of compression + tar only (avoids unbounded recursion) |
| Per-file preview / individual file download UI | out-of-model | no page; the ZIP bundle holds every file |

UX control patterns (sliders / color pickers / preset chips): **N/A** — this is a
file-input tool with no tunable parameters and no page; every competitor is
zero-config auto-detect, which the `Input::File`-only descriptor matches.

## Stated limits (also surfaced in the tool description / errors)

- Max 64 MiB compressed input; max 256 MiB decompressed; max 10,000 entries.
- ZIP entries must use Stored/Deflate (standard); other in-zip methods error clearly.
- Non-archive / unrecognised input errors with the list of supported formats.

## Verification

- Core unit tests: `detect` across all 7 formats + non-archive; plain zip; plain
  tar; single-stream gzip (embedded/hint naming + default name); the layered-tar
  family for all 5 compressors (gz/bz2/xz/zst/lz4, real encode→decode round-trip);
  zip path-traversal sanitization; empty/garbage errors. Block tests: output-name
  derivation, ext stripping, chat-schema drift guard. `cargo test --workspace` green.
- `wafer build` OK (all pure-Rust decoders instantiate under wasm32-wasip1;
  block.wasm 1198.9 KiB).
- CLI end-to-end: public ZIP (codeload octocat/Hello-World) → `master.zip`;
  public tar.gz (npm left-pad-1.3.0.tgz) → detected `tar (.tar.gz)`, 10 files,
  output ZIP verified valid via Python `zipfile`; non-archive URL → clean
  "unrecognised archive" error.
- No page → no wasm-pack/generator/Playwright step (matches sibling file tools).

# lzma-compress — competitor analysis & differentiation

**Tool:** `gizza-ai/lzma-compress` — compress a file into LZMA `.xz` (XZ container
around an LZMA2 filter), returned for download. The high-ratio sibling of
`gzip-compress`.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `xz` / `xz -9` (XZ Utils) | CLI | The reference implementation, but a terminal tool; replaces the file in place by default. |
| Let Compress (`letcompress.com`) | Web | In-browser WASM `.xz` (LZMA2); private, but a single-purpose ad-supported site. |
| ezyZip (TAR.XZ) | Web | Browser-side WASM, no upload; oriented at `.tar.xz` archives rather than a single stream. |
| 7-Zip / archive apps | App | GUI, supports `.xz`/`.7z`; heavyweight for one file. |
| Online "xz compressor" sites | Web | Most **upload your file to a server**; size caps; ads. |

## How gizza's tool is better / different

1. **Maximum-ratio format.** `.xz` (LZMA2) typically beats gzip/zip on size. On a
   680 KB text file it reached **82% smaller** vs gzip-9's ~76% (125 304 B vs
   162 912 B) — measurably tighter for the same input.
2. **Local — your file is never uploaded.** Runs in WASM (chat Service Worker +
   CLI) via the pure-Rust `lzma-rust2` encoder (a port of tukaani's xz-for-java).
   Privacy win over server-side web compressors.
3. **Standard output.** Produces a canonical `.xz` stream that `xz -t`, 7-Zip and
   any standard tool decompress — verified end-to-end (see below).
4. **Adjustable level.** `level` 0-9 (default 6) trades speed for size.
5. **Honest reporting.** Output states input→output bytes and the approximate
   size reduction, so an already-compressed input (little gain) is obvious.
6. **Any file via url or ref.** Compress a fetched URL or a `ref` from a prior
   tool call. Pairs with `gzip-compress` (faster, more compatible) by format.

## Implementation note — wasm adaptation (kept honest)

The reference `xz` presets 4-9 select the **BT4 (binary-tree)** match finder and a
16-64 MiB dictionary; both abort inside the wafer wasm runtime (verified: the CLI
panicked with a `wasm unreachable` at level 5+, then level 9 after the first fix).
The encoder is therefore configured wasm-safely **without weakening the output
format**:
- BT4 → **HC4 (hash-chain)** finder, kept in **Normal optimal-parse** mode with a
  useful search depth — so the high levels still beat gzip-9.
- Dictionary capped at **8 MiB** (the preset-6 size, the largest that
  instantiates). This only affects long-range matches in files *larger* than
  8 MiB; for the common case it is identical. Higher levels still differ via their
  larger `nice_len` / depth.

All ten levels now run and emit valid `.xz`. This is a runtime-fit adaptation of
parameters, not a custom container — output remains 100% standard XZ.

## Verification

- **Unit (6):** xz magic header (`FD 37 7A 58 5A 00`), round-trip via the crate's
  `XzReader`, repetitive data compresses >50×, all levels 0-9 round-trip, level
  clamping, empty input.
- **End-to-end CLI:** compressed a fetched 680 548-byte file (linux MAINTAINERS)
  at level 9 → 125 304-byte `.xz` (~82% smaller) that **system `xz -t`** validated
  and **`xz -dc`** restored to the exact 680 548 original bytes. All levels 0-9
  produced `xz -t`-valid output on a binary input.

## Surfaces & honest scope

- **Chat + CLI only — no web page** (file→file output, same no-page pattern as
  `gzip-compress` / `gunzip`).
- Single `.xz` stream (not a multi-file archive). For bundling many files use
  `create-zip`; for `.tar.xz` use the tar tools.

## Possible future enhancements

- A dedicated "compress pasted text → .xz" text-input variant.
- `.lzma` (legacy) container option, or a delta/BCJ pre-filter for binaries
  (`lzma-rust2` exposes both) — both fit the model as extra params.
- A matching `.xz` decompressor tool to complete the pair.

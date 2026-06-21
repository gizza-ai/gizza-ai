# gzip-compress — competitor analysis & differentiation

**Tool:** `gizza-ai/gzip-compress` — compress a file into gzip (.gz), returned for
download. The inverse of `gunzip`.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `gzip` / `gzip -9` | CLI | The reference, but a terminal tool; replaces the file in place by default (flags to keep it). |
| Online "gzip compressor" sites | Web | **Upload your file to a server**; ad-supported; size caps. |
| 7-Zip / archive apps | App | GUI, heavyweight for a single `.gz`. |
| Browser `CompressionStream('gzip')` | Web/lib | Need to write JS; no filename header; output handling is manual. |

## How gizza's tool is better / different

1. **Local — your file never uploaded.** Runs in WASM (chat SW + CLI) via
   `flate2` (miniz_oxide). Privacy win over web compressors.
2. **Pairs with `gunzip`.** Symmetric: gzip-compress makes the `.gz`, gunzip
   restores it. The **original filename is stored in the gzip FNAME header**, so
   gunzip recovers the real name automatically.
3. **Adjustable level.** `level` 1-9 (default 6) trades speed for size.
4. **Honest reporting.** Output states input→output bytes and the approximate
   size reduction, so you can see when a file is already compressed (little gain).
5. **Any file via url or ref.** Compress a fetched URL or a `ref` from a prior
   tool call.

## Verification

Core round-trips data through gzip→gunzip (including FNAME recovery) and confirms
repetitive data compresses >10×. **End-to-end CLI** compressed a fetched 558 541-
byte EPUB at level 9 → a valid `1F 8B` gzip that Python's `gzip.decompress`
restored to the exact 558 541 original bytes (≈2% gain — correct, since an EPUB
is already a compressed ZIP).

## Surfaces & honest scope

- **Chat + CLI only — no web page** (file→file output, same no-page pattern as
  `gunzip`).
- Single-member gzip (not multi-file). For bundling many files use `create-zip`;
  for `.tar.gz` use the tar tools.

## Possible future enhancements

- A dedicated "compress pasted text → .gz" text-input variant.
- Show the gzip CRC32 / mtime, or set a custom output filename.

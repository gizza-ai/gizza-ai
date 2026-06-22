# identify-archive-format — competitor analysis (2026-06-22)

## What the tool does
Detects the **compression or archive format** of an uploaded blob from its leading
magic bytes (independent of filename/extension) and answers the practical question
"which decompressor do I reach for?". Pure-Rust, dependency-free byte sniffing →
flat JSON: `format`, `name`, `mime`, `extension`, `kind`
(`archive` vs `compressor`), `decompress_with` (concrete command), `bytes`,
optional `filename`. Surfaces: **chat + CLI** (no page — file-input report, same
no-page pattern as `detect-file-type` / `pdf-extract-text`).

## Formats covered (in-model)
- **Compressors (single stream):** gzip, bzip2, xz, raw LZMA (.lzma alone),
  zstd, lz4 frame, lzip, lzop, zlib/deflate (structural CMF/FLG 31-multiple
  check), Unix compress `.Z` (LZW), old pack `.z`, Snappy framing.
- **Container archives:** ZIP, TAR (ustar@257), 7-Zip, RAR, Unix `ar`/`.deb`,
  cpio (ASCII + binary magics), Microsoft Cabinet (`MSCF`).
- Non-archive input → a clear "not a recognised archive/compression format"
  error rather than a wrong guess.

## Competitors surveyed (top 5)
1. **CyberChef "Detect File Type" / "Magic" operation** — the backlog's named
   competitor. In-browser, magic-byte based, very broad (all file types, not just
   archives). Outputs MIME + extension + description. No "which decompressor" hint,
   no archive-vs-compressor classification.
2. **`file(1)` / libmagic** — the canonical CLI magic-number identifier. Extremely
   broad and authoritative; outputs a free-text description. Not browser-local; no
   structured JSON; no decompression hint.
3. **TrID / online "what is this file" identifiers** — signature DB based, broad,
   web-upload (server-side). Returns a ranked list of candidate types.
4. **`infer` / `file-type` (npm)** — developer libraries that sniff magic bytes and
   return MIME + ext. Broad coverage; library, not an end-user tool; no command hint.
5. **7-Zip / `unar` "open and it figures out the format"** — practical archive tools
   that auto-detect on extraction. They detect to extract, not to *report*; require
   the file locally and the right tool installed.

## Gap analysis (fit-to-model)
| Capability | Competitors | This tool | Action |
|---|---|---|---|
| Magic-byte detection, browser-local, no upload | CyberChef ✓ | ✓ | met |
| Structured JSON (format/mime/ext) | infer/file-type ✓ | ✓ | met |
| **Archive-vs-compressor classification** | none | ✓ | **differentiator** |
| **Concrete `decompress_with` command hint** | none | ✓ | **differentiator** |
| Raw LZMA / zlib / .Z / lzip / lzop / snappy | partial (file ✓, CyberChef partial) | ✓ | met — closed via structural heuristics |
| cpio / ar / cab containers | file ✓ | ✓ | met |
| Clear "not an archive" answer (vs wrong guess) | varies | ✓ | met |
| Recursive/nested detection (e.g. `.tar.gz` → tar inside gzip) | file partial | reports outer (gzip) only | **out of scope** — caller decompresses then re-checks; documented |
| Full general file-type table (images/docs/etc.) | CyberChef ✓, file ✓ | intentionally NOT — that is `detect-file-type` | by design (scoped tool) |
| Listing archive *contents* | 7-Zip ✓ | no — see `unzip` block for ZIP listing | out of scope |

## Relationship to existing `detect-file-type` block
`detect-file-type` is a **general** sniffer (images/audio/video/docs/fonts/exes/
archives/text). `identify-archive-format` is **scoped to the compression question**
and is *not a strict subset*: it adds formats `detect-file-type` does not classify
(raw LZMA, zlib/deflate, Unix compress `.Z`, lzip, lzop, snappy, cpio, cab) and
reports archive-specific fields (`kind`, `decompress_with`) instead of a generic
category. It returns a focused error for non-archives rather than falling back to a
text/binary guess. Distinct purpose + distinct fields ⇒ kept as a separate tool, not
skiplisted.

## Out-of-model (not built)
- Recursive unwrapping of multi-layer streams (`.tar.gz`, `.tar.xz`) — reports the
  outer wrapper only; recursive unwrap would need actual decompression.
- Listing/extracting archive members — that is a separate decompress/unzip surface
  (see the `unzip` block).
- A standalone page — file-input→JSON report has no page render mode (consistent
  with `detect-file-type`).

## Verification (this run)
- `cargo test --workspace`: 9 tests pass (8 core sniffing + 1 chat-schema drift guard).
- `wafer build`: chat block builds and **validates/instantiates** (485 KiB).
- CLI: zip URL → `format=zip,kind=archive`; `.tar.gz` URL → `format=gzip,kind=compressor`;
  PNG URL → clear "not a recognised archive" error. All correct.
- Generator renders 155 tools with no error (no-page block, like detect-file-type).

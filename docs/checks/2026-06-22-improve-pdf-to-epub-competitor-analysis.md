# pdf-to-epub — competitor analysis & improvement snapshot (2026-06-22)

## Tool

`pdf-to-epub` — convert a text-based PDF into a reflowable EPUB ebook. Pure Rust
(`lopdf` text extraction + `zip` EPUB-container assembly), runs on all backends
including the chat Service Worker. Surfaces: **chat + CLI** (no standalone page —
a binary file input with binary EPUB output fits neither the pure-text page nor
the ffmpeg file→media page shape; the no-page file-input pattern shared with
`images-to-pdf` and `epub-to-markdown`).

## Surfaces verified

- **Chat block**: `wafer build` validates + instantiates the wasm32-wasip1
  `block.wasm` (1710 KiB). Pure-Rust, so it runs in the chat SW.
- **CLI**: `gizza tool pdf-to-epub url=… title=… author=…` fetches a public PDF,
  extracts text, and writes a valid `book.epub`. Verified the output with
  `zipfile`: `mimetype` is the first entry and **stored** (uncompressed) per the
  OCF spec, the OPF has the title/author/identifier, the spine lists every page,
  and `testzip()` passes. Verified both `preserve_line_breaks=false` (default)
  and `=true`.
- **Drift guard**: `schema_json_matches_authored_chat_schema` asserts the
  descriptor-derived chat schema equals the authored JSON (no LLM-facing drift).
- No page surface to Playwright (stated explicitly, not skipped silently).

## Top competitors surveyed

1. **Calibre `ebook-convert`** (desktop, open source) — the reference-quality
   converter. Heuristic processing: line-unwrapping (punctuation + line-length
   clues), chapter/scene-break detection, automatic TOC, metadata insertion,
   proportional font-size adjustment. Notes "PDF is one of the worst formats to
   convert from."
2. **CloudConvert** — 200+ formats, web API, security-focused; paid tiers for
   larger files.
3. **Zamzar** — 1200+ conversion types, simple UI, email delivery, size-gated
   free tier.
4. **Convertio** — broad format matrix, free-tier file-size limit.
5. **PDF24 / online2pdf / UPDF** — free browser converters, drag-and-drop, no
   install; minimal structural control.

## Capability diff & gaps (fit-to-model)

| Capability | Competitors | pdf-to-epub | Action |
|---|---|---|---|
| PDF text → EPUB | all | yes | shipped |
| Valid OCF EPUB (mimetype stored first) | all | yes | shipped |
| Book metadata (title/author) | Calibre, most | yes (`title`, `author` → dc:title/dc:creator) | shipped |
| Table of contents | Calibre, most | yes (EPUB 3 nav.xhtml + EPUB 2 toc.ncx) | shipped |
| Line-unwrapping for clean reflow | Calibre (heuristic) | **added** — wrapped lines merged into flowing paragraphs by default; `preserve_line_breaks=true` keeps the original layout (poetry/code) | **closed this run** |
| Deterministic output | (none advertised) | yes (fixed ZIP timestamp + derived stable id) | shipped (differentiator) |
| Per-page chapter split + page nav | varies | yes (one chapter per PDF page) | shipped |
| Privacy / local conversion | mixed (most upload to a server) | runs locally in chat SW / CLI | shipped (differentiator) |

### Out-of-model gaps (NOT built — listed, not attempted)

- **OCR of scanned/image-only PDFs** — needs an OCR model (out of the pure-Rust
  + ffmpeg model). Documented as an explicit limitation in the skill description.
- **Embedded image / figure extraction into the EPUB** — would require decoding
  and re-embedding PDF image XObjects; significant scope, low marginal value for
  a text-reflow ebook, deferred.
- **Heuristic chapter/scene-break detection** (Calibre-style) — NLP-ish heuristic
  over content; brittle and non-deterministic. Page-per-chapter is a predictable
  substitute; deferred.
- **Font-size / CSS styling adjustment** — readers reflow EPUB anyway; deferred.

## Improvement applied this run

Added the `preserve_line_breaks` boolean (default `false`). By default, lines
that a PDF hard-wraps inside a paragraph are now **unwrapped** into flowing text
so the EPUB reflows cleanly on any reader (the single biggest real-world quality
gap vs. Calibre). Setting it `true` keeps every PDF line break as an explicit
`<br/>` for content where layout matters (poetry, code, tables). Covered by a
core unit test (`line_unwrap_vs_preserve`) and re-verified on both CLI paths.

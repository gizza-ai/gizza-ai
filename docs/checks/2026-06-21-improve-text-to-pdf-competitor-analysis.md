# text-to-pdf — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/text-to-pdf` — generate a clean, paginated PDF from plain text
with configurable font size and margins. Pure-Rust (`lopdf`, built-in Courier
font, no embedding). Text input → PDF output, so chat + CLI, no page (PDF bytes
output has no page render mode — like the other PDF/SVG-producing tools).

## What competitors do

- **Online "text to PDF" converters** — paste/upload text, download a PDF.
  **Weakness: the text is uploaded** to a server; free tiers add watermarks or
  caps.
- **`enscript` / `paps` / `pandoc` / `wkhtmltopdf`** — local + powerful, but need
  installing native tools (and a LaTeX/HTML pipeline for pandoc/wkhtmltopdf).
- **"Print to PDF"** — built into every OS, but manual (open the text, print,
  choose PDF), not scriptable or callable from chat.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`lopdf`) compiled to wasm: runs
   in the chat Service Worker and headless in the CLI. The text never leaves the
   device.
2. **Real pagination + wrapping.** Long lines wrap to the text width at word
   boundaries (hard-splitting over-long words), and content flows across as many
   US-Letter pages as needed — not a single clipped page.
3. **Configurable, predictable layout.** `font_size` and `margin` (in points) are
   adjustable; the built-in **Courier** monospace font means layout is exact and
   the PDF needs **no embedded font**, so output stays tiny (a 200-line doc is
   ~22 KB / 10 pages).
4. **Standards-compliant output.** A normal multi-page PDF (Type1 base font, one
   content stream per page) that opens in any viewer.
5. **Agent- + CLI-friendly.** One call from chat or `gizza tool text-to-pdf
   text=… --out file.pdf`; the PDF is a chainable `ref` for the other PDF tools
   (e.g. protect-pdf, merge-pdf).

## Honest scope

- **Monospace (Courier), Latin-1 text.** Uses a built-in font, so non-Latin
  scripts (CJK, Arabic, etc.) aren't rendered (those code points become `?`); this
  keeps the PDF dependency-free and tiny. Rich formatting (bold, headings, fonts)
  is out of scope — it's plain text → clean PDF.
- **US Letter pages.** Page size is fixed at Letter (612×792 pt).
- **No page (chat + CLI).** PDF-bytes output doesn't fit the page's text/field or
  media-render model.

## Tests

5 core unit tests: produces a valid `%PDF-` one-page document; **paginates** a
500-line input across multiple pages; **wraps** a 200-char word onto ≥3 lines all
within the width; wraps at **word boundaries** keeping words intact; and errors on
an out-of-range font size and an over-large margin. Plus the block drift-guard
schema test. **CLI verified** end-to-end (a 200-line input → a valid 10-page PDF
using Courier). `wafer build` instantiates the chat block (387 KiB).

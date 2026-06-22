# markdown-to-pdf — competitor analysis (2026-06-21)

Tool: `gizza tool markdown-to-pdf` — converts a Markdown document into a formatted,
paginated PDF. Pure-Rust (`pulldown-cmark` + `lopdf`, base-14 Helvetica/Courier fonts),
so it runs on every backend incl. the chat Service Worker. Surfaces: **chat + CLI**
(text input → PDF bytes → no standalone page, like `text-to-pdf`).

## Surfaces verified
- **Chat block**: `wafer build` validates `block.wasm` instantiates in wasm32-wasip1 (628.7 KiB). OK.
- **CLI**: `gizza tool markdown-to-pdf markdown=… [font_size=] [margin=] [page_size=]` →
  valid `%PDF-1.5`. Content-stream decode confirms headings, bold/italic runs, list
  markers (incl. nested), code blocks, block quotes, and table cells all render.
  Error paths (bad font_size, oversized margin, unknown page_size) reject cleanly.
- **Page**: none — PDF-bytes output has no page render mode (consistent with the
  other `*-to-pdf` blocks).

## Competitor landscape

| Tool | Input → Output | Notable features |
|------|----------------|------------------|
| md2pdf (md2pdf.netlify.app) | paste/upload MD → PDF | live preview, GitHub-style CSS theme, code blocks |
| Dillinger | MD editor → export HTML/PDF/Styled PDF | live split preview, cloud sync (Dropbox/Drive), styled export |
| CloudConvert | upload `.md` → PDF | server-side batch, many formats, API; uploads leave the device |
| markdowntopdf.com | paste/upload MD → PDF | one-click, GitHub-flavored styling, server-side |
| Pandoc (CLI) | MD → PDF (via LaTeX/wkhtmltopdf) | the reference: templates, TOC, math, citations, custom fonts; needs a TeX/HTML engine |
| VS Code `markdown-pdf` ext | MD file → PDF/HTML/PNG | headers/footers, highlight.js syntax colors, mermaid, custom CSS (Chromium-rendered) |

## Gap analysis (fit-to-model)

**In-model — implemented / present:**
- Headings (H1–H6 scaled + bold), paragraphs, **bold/italic/bold-italic**, inline `code`. ✔
- Ordered + unordered **nested** lists with bullets/numbers. ✔
- Fenced & indented **code blocks** (monospace, verbatim newlines). ✔
- **Block quotes** (indented), **horizontal rules**, **tables** (flattened rows), **task lists**. ✔
- GitHub-flavored extensions enabled (tables, strikethrough, task lists, footnotes). ✔
- **Configurable** base font size + page margin. ✔
- **Page size** `letter` / `a4` — added this run to match the near-universal competitor option. ✔
- **Privacy**: runs fully locally; the document never leaves the device — a real edge over
  the server-side converters (CloudConvert, markdowntopdf.com). ✔
- Automatic pagination across as many pages as needed. ✔

**Out-of-model (documented, not built) — would need deps/engines gizza deliberately avoids:**
- **Syntax-highlight colors** in code blocks — needs a highlighter + RGB color ops; base-14
  mono is monochrome by design. (Could be added later via `syntect`, but heavy.)
- **Embedded/custom web fonts** — base-14 fonts only (no font embedding keeps the PDF tiny
  and deterministic). Non-Latin-1 glyphs fold to `?`.
- **Raster image embedding** (`![alt](url)`) — we render alt text only; embedding needs an
  image decoder + XObject pipeline and network fetch. Out of scope for a pure text→PDF block.
- **Mermaid / diagram rendering**, **full LaTeX math** ($…$), **headers/footers & page
  numbers**, **clickable link annotations**, **table-of-contents**, **arbitrary CSS theming** —
  all require a browser/LaTeX rendering engine (the VS Code ext uses Chromium; Pandoc uses
  TeX). gizza is pure-Rust + ffmpeg, so these stay out of model.
- **Batch / multi-file** — single document per call by design.

## Conclusion
The tool covers the full set of in-model Markdown structure that a pure-Rust, no-engine
converter can render, plus the configurable page-size/font/margin knobs that distinguish the
mainstream converters, and is privacy-first by construction (no upload). The remaining
competitor features (syntax colors, embedded fonts/images, mermaid, LaTeX math,
headers/footers, CSS themes) all require a rendering engine gizza intentionally does not ship,
and are listed here rather than built. No competitor copy, branding, or trademarks were used.

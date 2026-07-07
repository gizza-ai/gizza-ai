# pdf-to-markdown — competitor analysis (2026-07-06)

Tool function: convert a PDF into clean, **structured** Markdown (headings, lists,
paragraphs) rather than the flat text dump that the sibling `pdf-extract-text` block
produces. This block is the "give me Markdown I can paste into a doc/LLM" surface.

## Not a duplicate of `pdf-extract-text`

`pdf-extract-text` returns the raw selectable text layer (lopdf `extract_text_chunks`
concatenated). It does **no** structure: no heading levels, no list markers, no line
reconstruction. `pdf-to-markdown` re-implements extraction to also track per-run font
size and text-positioning (`Tf`/`Tm`/`Td`/`T*`/`'`/`"`), so it can emit `#`/`##`/`###`
headings from document-wide font-size statistics, `-`/`1.` list items, and
paragraph-joined body text with de-hyphenation. Different output, different job —
confirmed by reading `blocks/pdf-extract-text/core/src/lib.rs`.

## Competitors scanned (paraphrased — no copy/branding reproduced)

1. **CopyMarkdown (copymarkdown.com/pdf-to-markdown)** — browser-side, free, no
   sign-up. "Headings become Markdown headings, bullet lists stay as lists, tables
   become Markdown tables, paragraphs flow cleanly." Minimal controls: upload + copy /
   download `.md`. 20 MB cap. FAQ states it works only on text-layer PDFs; scanned /
   image-only PDFs need OCR (on roadmap).
2. **jzillmann/pdf-to-markdown (open source, pdf.js based)** — client-side. Detects
   heading levels from **document-wide font-size statistics** (rank distinct sizes),
   reconstructs lines/paragraphs from text positioning, de-hyphenates wrapped words,
   emits lists. This is the exact heuristic pipeline a pure (no-ML) converter uses and
   the design this block follows.
3. **iamarunbrahma/pdf-to-markdown** — headings "from document-wide font-size
   statistics"; ordered/unordered/nested lists; monospace→code blocks; ruled and
   borderless tables as GFM pipe tables; math→LaTeX (heuristic, optional ML extra);
   multi-column reading order; header/footer removal. Documented limits: no OCR for
   scanned pages; math is best-effort; dense multi-table / RTL pages need manual review.
4. **PDFNano / LightPDF (feature pages)** — "intelligently detects headings, bullet
   points, tables, inline styles"; GitHub-flavored Markdown tables; 10–20 MB caps;
   text-layer only.

## Table-stakes → in-model / out-of-model decisions

| Capability | Decision | Where it lands |
|---|---|---|
| Heading detection from font-size statistics + ranked levels | IN-MODEL | core heading logic; H1–H6 by size rank vs. body mode |
| Unordered lists (•, ‣, ◦, –, —, -, * markers → `-`) | IN-MODEL | `detect_lists` (default on) |
| Ordered lists (`1.`, `1)`, `a.`, `a)` → `1.`) | IN-MODEL | `detect_lists` |
| Paragraph reconstruction (join wrapped lines) | IN-MODEL | core line grouping by Y-gap |
| De-hyphenation of line-wrapped words | IN-MODEL | `dehyphenate` (default on) |
| Page range / single page | IN-MODEL | `page` (1-based, omit = all) |
| Page-break separator between pages (`---`) | IN-MODEL | `page_separator` = `rule`\|`blank` |
| File-size cap (10–20 MB across competitors) | IN-MODEL | 16 MiB input cap |
| `'`/`"` text operators (lopdf's own extractor drops them) | IN-MODEL (correctness win) | core handles them as show-on-next-line |
| Tables → GFM pipe tables (merged/borderless cells) | OUT-OF-MODEL | stated limit; a dedicated `pdf-table-extract` block already exists. Requires robust X-column clustering across borderless cells — not reliable as a pure heuristic within budget. Table text still appears as plain lines. |
| Inline bold/italic styling | OUT-OF-MODEL | stated limit; needs per-glyph font-weight/style mapping. Bold *headings* are still captured via size. |
| Code blocks via monospace-font detection | OUT-OF-MODEL | stated limit |
| Multi-column reading order | OUT-OF-MODEL | stated limit (text emitted in content-stream order) |
| Header/footer removal | OUT-OF-MODEL | stated limit (needs cross-page repetition detection) |
| Math → LaTeX | OUT-OF-MODEL | ML/heuristic; out of scope for a pure block |
| Images / OCR of scanned PDFs | OUT-OF-MODEL | universal competitor limit — no text layer, no OCR. See `extract-pdf-images` for embedded images. |

## UX control patterns

Real competitors ship **minimal** controls for this tool class (upload + copy /
download; no sliders, color pickers, or preset chips). CopyMarkdown exposes zero
parameters; others at most a page range. So the in-model control surface is small and
maps cleanly to descriptor params (`page`, `page_separator`, `detect_lists`,
`dehyphenate`) — no page UI patterns (this is a binary-file-input tool: chat + CLI,
no page, matching the sibling `pdf-extract-text` / `epub-to-markdown` / `pdf-to-epub`).

## Surface decision

Binary-file (PDF) in, Markdown text out → **no page** (the established F3 file-input
pattern; `pdf-extract-text`, `epub-to-markdown`, `pdf-to-epub`, `extract-pdf-images`,
`pdf-table-extract` all ship chat + CLI only, no `page/`, no `web/`). Verified surfaces:
unit tests (synthetic PDFs, exact output) + `wafer build` (chat block.wasm instantiates)
+ CLI against a public PDF. No wasm-pack / generator-page / Playwright — there is no
page to headlessly verify, stated explicitly per the honesty gate.

# markdown-to-docx — competitor analysis (2026-07-17)

Scan done BEFORE implementing the new tool. Sources paraphrased only — no competitor
copy, branding, or trademarks reproduced.

## Competitors scanned (top real tools)

1. **markdownlivepreview.dev — Markdown to Word** — pure in-browser converter, no options panel.
   Supports: GFM tables, fenced code blocks (monospace + background shading, language highlighting),
   native clickable Word hyperlinks, embedded images, blockquotes (left border + indent), inline/block
   LaTeX math → Word equation objects, bold/italic/strikethrough/underline/highlight,
   superscript/subscript (`^x^`/`~x~`), ordered + unordered lists, page breaks via `---`. No
   configurable settings. No stated limits ("unlimited").
2. **markdowntoword.io** — paste → preview → download DOCX, no configuration exposed. Supports
   headings H1-H6, ordered/unordered + nested lists, GFM tables, fenced code blocks, blockquotes,
   images, LaTeX math, bold/italic/links. No page-size/font/margin controls. No stated numeric limits.
3. **digitaltoolpad.com — Markdown to DOCX** — fully client-side ("zero uploads", no length limit).
   Supports GFM, code blocks with 30+ language highlighting, tables → Word tables, images (public
   URLs). Hyperlinks/blockquotes/task lists not documented. No configuration panel.
4. **mconverter.eu** (cross-check) — batch file converter; free tier ≤15 files/day, ≤8 at once,
   ≤100 MB/file. Format-conversion focused, no per-document styling controls.

## Table-stakes → in-model / out-of-model

| Feature | In-model? | Decision |
| --- | --- | --- |
| Headings H1–H6 → Word heading styles | in-model | ✅ built (styled `Heading1`–`Heading6`) |
| Bold / italic / strikethrough / inline `code` | in-model | ✅ built (multi-run inline parser) |
| Ordered + unordered + nested lists | in-model | ✅ built (`numbering.xml`, bullet + decimal, 5 indent levels) |
| GFM tables → Word tables | in-model | ✅ built (`w:tbl`, header shading, alignment) |
| Fenced code blocks (monospace + shading) | in-model | ✅ built (Consolas, grey shading, per-line) |
| Blockquotes (indent + left border) | in-model | ✅ built (`Quote` style, left border, indent) |
| Hyperlinks → native clickable Word links | in-model | ✅ built (real `w:hyperlink` + doc relationships) |
| Task lists `- [ ]` / `- [x]` | in-model | ✅ built (☐ / ☑ glyph prefix) |
| Horizontal rule `---` → line / page break | in-model | ✅ built (bottom-border rule; `page_break` option maps `---`→page break) |
| Page size (A4 / Letter) — **config gap most competitors lack** | in-model | ✅ built (`page_size` enum, `sectPr` `pgSz`) |
| Body font family — **config gap** | in-model | ✅ built (`font_family` enum → `styles.xml`) |
| Base body font size — **config gap** | in-model | ✅ built (`font_size` number, points) |
| Document title metadata + title paragraph | in-model | ✅ built (`title` param → `dc:title` + `Title` style paragraph) |
| Embedded images from URLs | **out-of-model** | listed only — needs network fetch of remote binaries; this is a pure, offline, no-fetch block. Image syntax renders as its alt text. |
| LaTeX math → Word equation (OMML) objects | **out-of-model** | listed only — a full TeX→OMML engine is far outside a line-oriented converter. |
| Mermaid diagram rendering | **out-of-model** | listed only — needs a JS rendering engine + raster embed. |
| Code syntax highlighting (per-language colors) | **out-of-model** | listed only — a multi-language highlighter is disproportionate; code is monospace + shaded instead. |
| Table of contents field | considered, deferred | Word TOC is a field that only populates on the user pressing "update field"; low value without heading numbering — deferred, not built. |

Positioning angle: competitors are almost all zero-config "paste and download." Our differentiator is
**real Word settings** (page size, body font, font size) plus faithful GFM structure — all still
100% in-browser/offline.

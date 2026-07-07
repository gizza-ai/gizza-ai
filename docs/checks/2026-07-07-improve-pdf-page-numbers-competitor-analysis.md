# pdf-page-numbers — competitor analysis (2026-07-07)

Built the new gizza tool `pdf-page-numbers` (stamp page numbers onto an existing
PDF). This snapshot records the competitor scan that shaped the descriptor.
All notes are **paraphrased** — no competitor copy, branding, or trademarks are
reproduced. Out-of-model items are listed, not built.

## Tool shape

Document-in → PDF-out, so it follows the gizza PDF family norm: **chat + CLI only,
no standalone page** (a binary file upload + a downloadable PDF has no page render
mode — the same as all 22 existing PDF tools, e.g. `pdf-rotate`, `pdf-split`).
Pure-Rust via `lopdf` 0.36 (no font embedding; base-14 Type1 fonts), so it runs on
every backend including the chat Service Worker.

## Competitors scanned (top 5 real tools)

| tool | positions | format templates | numbering styles | start # | page range | font / size / color | margin | opacity |
| ---- | --------- | ---------------- | ---------------- | ------- | ---------- | ------------------- | ------ | ------- |
| iLovePDF | 6-cell top/bottom × L/C/R | number / "Page {n}" / "Page {n} of {p}" / custom template | decimal only | yes ("first number") | rich: exclude first/last N, cover-page toggle, custom range | family + size + color (+ background, shadow) | presets S/M/L | yes (opacity) |
| Smallpdf | top/bottom + four corners | plain sequential only (no templates advertised) | decimal only | yes | not advertised | none advertised | adjustable | no |
| Adobe Acrobat online | 6-cell top/bottom × L/C/R | not advertised | decimal (implied) | start-page choice (no explicit first-number) | all / range | none advertised | none | no |
| PDF24 | 9-anchor grid (adds vertical middle row) | "pattern" field (templates not enumerated) | decimal (implied) | yes ("first number") | first/last page fields | family (sans/serif/mono) + size + color | "space" in mm | yes (alpha) |
| PDF Toolbox | header/footer × L/C/R | bare / prefix+number / "Page X of Y" toggle | decimal; roman only via manual two-pass workaround | yes | thumbnail subset selection | color only | mentioned, no field | no |

## Table-stakes → decision

Every table-stake lands in the descriptor OR is listed below.

- **Position grid (top/bottom × left/center/right)** — IN. `position` enum, 6 cells,
  default `bottom-center`.
- **Format templates ("Page X of Y", "X / Y", bare number, prefix/suffix)** — IN.
  A `format` template with `{n}` (current) and `{total}` (last printed) placeholders
  subsumes every competitor preset and the prefix/suffix pattern in one field.
- **Starting number** — IN. `start_number` (default 1); the value printed on the
  first *stamped* page.
- **Page range / skip cover page** — IN. `pages` spec ("all", "1,3-5", or open
  "2-" to skip a cover) — mirrors iLovePDF's exclude-first / cover-page and
  Adobe's range in one 1-based selector.
- **Font family** — IN, base-14 only. `font` enum helvetica (sans) / times (serif) /
  courier (mono) — the same three classes PDF24 exposes, no embedding needed.
- **Font size** — IN. `font_size` points (4–144).
- **Font color** — IN. `color` hex (short `#f00` and long `#ff0000` forms).
- **Margin** — IN. `margin` in points (0–400; 72 pt = 1 in) — a numeric field is
  more precise than the S/M/L presets or mm "space".
- **Opacity / transparency** — IN (2 of 5: iLovePDF, PDF24). `opacity` 0.05–1.0 via a
  PDF ExtGState — enables faint, watermark-style numbers.

## Differentiator we ship that none of the five do natively

- **Roman-numeral and letter numbering** — `style` enum: decimal, roman-lower
  (i, ii, iii), roman-upper (I, II, III), alpha-lower (a, b, c), alpha-upper
  (A, B, C). Every competitor is decimal-only; PDF Toolbox only offers a manual
  two-pass workaround for roman front-matter. Combined with `pages` + `start_number`,
  a user can number front-matter i, ii, iii and the body 1, 2, 3 in two calls —
  natively, no workaround.

## Considered, rejected (in-model but declined)

- **9-anchor grid with a vertical "middle" row** (PDF24 only, 1/5) — page numbers in
  the vertical center overlap body text and no other competitor offers it; the 6-cell
  top/bottom grid is the real use case. Enum kept lean.
- **Font shadow** (iLovePDF only) — single-vendor cosmetic; adds a second draw pass
  and offset params for marginal value.
- **Background color box behind the number** (iLovePDF only) — single-vendor; a
  filled rect + the legibility tuning it implies is scope the core feature set
  doesn't need.

## Out-of-model (needs a backend / account / font embedding)

- **Extra font families** (Impact, Comic Sans, Verdana, Arial Unicode, Devanagari,
  etc. from iLovePDF/PDF24) — need embedded font programs; base-14 Helvetica/Times/
  Courier cover the standard sans/serif/mono classes without embedding.
- **Cloud import (Google Drive / Dropbox)** and **save-to-cloud / share links**
  (iLovePDF, Adobe) — need accounts + a backend.
- **Batch numbering of many PDFs at once behind a paid tier** (Smallpdf) — one PDF
  per invocation; the chat agent can loop calls instead.
- **Live thumbnail preview / drag-and-drop subset picker** (Smallpdf, PDF Toolbox,
  PDF24) — page-UI features; this tool has no standalone page (family norm). The
  `pages` spec covers the underlying subset capability on chat + CLI.

## Known limitations (stated honestly)

- Numbers are placed relative to the **unrotated MediaBox**; a page carrying a
  `/Rotate` entry may show the number in unrotated page space.
- `{total}` renders the **largest number that will be printed** (start_number +
  stamped-page-count − 1), so "n of total" stays self-consistent even when a cover
  page is skipped or a custom start is used.

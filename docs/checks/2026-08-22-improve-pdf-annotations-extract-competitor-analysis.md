# pdf-annotations-extract — competitor analysis (2026-08-22)

Scan run **before** implementing, per `create-next-tool` step 4. Everything below is
**paraphrased** from public tool pages — no competitor copy, branding, or trademarks are
reproduced anywhere in this repo. Out-of-model items are *listed*, never built.

Search: "extract PDF annotations comments highlights online tool export" (WebSearch, 2026-08-22).
Four reachable competitor tools were profiled (the brief asks for three; a fourth was kept
because the highlight-only tool turned out to be thin).

## Competitor profiles (paraphrased)

### C1 — pdfannotations.com (annotation export suite)
- **Extracts:** highlights, underlines, squiggly/wavy underlines, strikeouts, sticky notes /
  comments.
- **Per-annotation fields:** page number, author, date, colour, comment text, and the
  highlighted/marked-up text itself.
- **Options:** free tier has no filters; the paid tier filters by annotation type, colour,
  author, and page range, and lets the user review/deselect annotations before export.
- **Output:** free — Markdown, CSV, plain text; paid — XLSX, JSON, Notion, Obsidian, custom
  templates.
- **Limits:** none published; runs in-browser via WebAssembly, no upload.
- **UX:** drag-and-drop, a demo/sample document, a review list, one-click copy/download per
  format.

### C2 — pdfhighlightextractor.com (highlight-focused)
- **Extracts:** highlights, sticky notes, "area" annotations.
- **Options:** checkboxes to pick which of the three categories to pull.
- **Output:** CSV (spreadsheet formats announced as coming).
- **Per-annotation fields:** not documented on the page.
- **Limits / UX:** none stated; no sorting, filtering, or grouping.

### C3 — elysiatools.com (PDF annotation export)
- **Extracts:** highlights, underlines, strikeouts, text notes, free-text annotations, stamps,
  links, and shape markups.
- **Options:** single PDF upload (max 100 MB, `application/pdf` only) plus one toggle that adds
  the page number to each annotation.
- **Output:** one structured JSON string rendered into the page (no file download).
- **Per-annotation fields:** page (when the toggle is on), annotation subtype, author,
  annotation text, colour as a hex value.
- **UX:** client-side processing, plain form, output textarea for copy-to-clipboard, worked
  sample runs shown on the page.

### C4 — creshy.com (annotation & comment extractor)
- **Extracts:** reviewer comments/notes, sticky notes, highlighted text and markup regions;
  states that non-standard/vendor-specific annotation flavours may not be recognised.
- **Options:** file picker plus an in-results search box.
- **Output:** TXT, CSV, JSON, Markdown.
- **Per-annotation fields:** comment text, page location, author/metadata when present,
  annotation type.
- **Limits:** stated as device/browser-memory bound; some PDF structures may block full
  extraction.
- **UX:** drag-and-drop, a scan/progress phase, a consolidated result list, search + filter,
  copy-to-clipboard, mobile-responsive layout.

## Table-stakes → where each one landed

| # | Table-stake (seen at ≥1 competitor) | Verdict | Where it lives |
| - | ---------------------------------- | ------- | -------------- |
| 1 | Highlight / underline / strikeout / squiggly extraction | **in-model** | `types` enum + `kind` classification in `core` |
| 2 | Sticky notes (`Text`) + free-text callouts | **in-model** | `note` / `freetext` kinds |
| 3 | Drawings (ink, square, circle, line, polygon/polyline) | **in-model** | `drawing` kind |
| 4 | Stamps and links | **in-model** | `stamp` / `link` kinds |
| 5 | Page number per annotation | **in-model** | always emitted (`page`); no toggle — C3's toggle is a downgrade, page number is free |
| 6 | Author (`/T`) | **in-model** | `author` field |
| 7 | Date (`/M`) | **in-model** | `date` field, PDF `D:…` normalised to ISO-8601 |
| 8 | Colour as hex | **in-model** | `color` field, `/C` gray/RGB/CMYK → `#rrggbb` |
| 9 | Comment text (`/Contents`) | **in-model** | `comment` field |
| 10 | **The highlighted/marked-up text itself** (C1, C4) | **in-model** | `include_marked_text` (default on): a positioned text-layer walk maps `/QuadPoints` back onto the characters they cover |
| 11 | Filter by annotation type | **in-model** | `types` enum (`all`, `markup`, `highlight`, `underline`, `strikeout`, `squiggly`, `note`, `freetext`, `drawing`, `stamp`, `link`) |
| 12 | Filter by author | **in-model** | `author` (case-insensitive substring) |
| 13 | Filter by page range | **in-model** | `pages` (`"1,3-5"` spec, same syntax as `pdf-split`) |
| 14 | Sorting / grouping of results | **in-model** | `sort` enum (`page`, `author`, `type`, `date`) |
| 15 | Export as JSON / CSV / Markdown / plain text | **in-model** | `format` enum, all four |
| 16 | Drop empty/no-comment annotations | **in-model** | `include_empty` (default false — bare links and empty popups are noise) |
| 17 | Stated file-size limit | **in-model** | 16 MiB cap, stated in the schema + errors (C3 quotes 100 MB; our wasm sandbox is 64 MiB total) |
| 18 | Multi-select type filter (checkbox set, C2) | **considered, rejected** | a single-choice `types` enum plus `markup`/`drawing` grouping covers the real cases without a combinatorial free-text param; noted here rather than dropped |
| 19 | Review/deselect individual annotations before export (C1) | **out-of-model** | needs a stateful review UI; this block is a chat/CLI transform, not an app |
| 20 | XLSX / Notion / Obsidian / custom templates (C1) | **out-of-model** | account-bound or template-engine features; Markdown output is the in-model answer, and `csv-to-xlsx` already exists for the spreadsheet hop |
| 21 | In-results search box (C4) | **out-of-model** here | that is a page/UI affordance; this tool has no page (binary PDF in → text out), so the `author`/`types`/`pages` filters are the equivalent |
| 22 | Sample/demo document (C1, C3) | **in-model, done differently** | the generated CLI example + the worked example in the skill description play this role for a no-page block |

## Design decisions taken from the scan

- **Page number is unconditional.** C3 gates it behind a toggle; it costs nothing here and every
  other competitor reports it, so it is always present.
- **Four output formats out of the gate** (`json`, `csv`, `markdown`, `text`) — C1 and C4 both
  ship the same four in their free tiers, so anything less is a gap on day one.
- **Marked-up text is the differentiator.** C2/C3 do not recover the text under a highlight at
  all; C1 gates the richest export behind a paid tier. Implementing the `/QuadPoints` → text-layer
  mapping was spiked first (lopdf content-stream walk with per-code font widths from `/Widths`
  and Type0 `/W`, falling back to base-14 metrics) and confirmed feasible in-model before being
  promised here.
- **Accuracy is stated, not hidden.** Marked-up text is reconstructed by position, so a highlight
  whose box clips a glyph can gain or lose an edge character, and a scanned/image-only PDF has no
  text layer to recover. Both limits are in the schema description and the error copy rather than
  left for a user to discover.
- **`Popup` and `Widget` subtypes are skipped.** A `Popup` is only the on-screen container for
  its parent markup annotation (it would double every comment), and `Widget` is an AcroForm field
  — already covered by the existing `pdf-form-data-extract` block.

## No-page note

A PDF is a binary file input and the output is delimited text/JSON, so this is a chat + CLI
block with no standalone page — the same family shape as `pdf-extract-text`,
`pdf-form-data-extract`, `pdf-table-extract`, and `pdf-notes-outliner`. Playwright is therefore
not applicable; the CLI plus the descriptor drift-guard are the verifiable surfaces.

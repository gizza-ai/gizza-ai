# pdf-table-extract — competitor analysis (2026-07-06)

Tool: detect tables in a text-based PDF and export them as CSV / TSV / JSON,
preserving rows and columns. Chat + CLI surface only (no standalone page — a PDF
is a binary file input processed by pure-Rust wasm, and there is no non-ffmpeg
file-input page path in the generator; same shape as `pdf-extract-text` and
`xlsx-to-csv`).

## Competitors surveyed (paraphrased — no copy/branding reproduced)

1. **Tabula** (open-source desktop) — user draws a box around a table; exports
   CSV / TSV / JSON / script. Text-based PDFs only (no OCR). Two detection
   modes: lattice (uses ruling lines) and stream (uses whitespace/coordinates).
2. **A browser-local "PDF table extractor"** — extracts table-like rows into CSV
   or JSON in-browser using text coordinates, with page selection and fully
   local export.
3. **A multi-format PDF table exporter** — auto-detects tables and exports to
   CSV, JSON, XML, HTML, DOCX; lets the user add/remove/extend the detected table
   region before export.
4. **An OCR/AI extractor** — uses OCR + ML to pull tables out of scanned images
   and photos into Excel/CSV/JSON.

## Table-stakes → decision

| Capability | In/out of model | Where it landed |
|---|---|---|
| Export to CSV | in-model | `format=csv` (default) |
| Export to JSON | in-model | `format=json` — array of `{page, rows}` |
| Export to TSV / choose delimiter | in-model | `delimiter=comma\|semicolon\|tab` (tab → `.tsv`) |
| Preserve row/column structure | in-model | coordinate clustering → grid |
| Page selection | in-model | `page` (1-based; omit = every page) |
| First-row-as-header | in-model | `header=true` → JSON rows become keyed objects |
| Auto table detection from text coordinates | in-model | content-stream text-matrix positioning + row/column clustering |
| Text-based PDFs only (no OCR) | shared limitation | documented — OCR is **out of model** (needs an ML model; gizza is pure-Rust + ffmpeg) |
| Manual box-draw / edit detected region | out of model | requires an interactive canvas UI; not built. Auto-detection only. |
| Lattice (ruling-line) detection | out of model (v1) | we use the whitespace/coordinate ("stream") method only |
| Export to XML / HTML / DOCX | out of model (v1) | CSV/TSV/JSON cover the ask in the backlog description; not built |

Every table-stake is either implemented or explicitly listed above as out of
model — none dropped silently.

## UX control patterns

Competitors offer a page picker, an output-format toggle, a delimiter choice,
and a header toggle. Because this is a no-page (chat + CLI) tool, those map to
descriptor params (`page`, `format`, `delimiter`, `header`) rather than page
controls/preset chips. Each param has an LLM-actionable `.describe()`, and every
fixed-choice param (`format`, `delimiter`) is a `Param::enumv`.

## Method + honest limitations

`lopdf`'s convenience `extract_text` discards text coordinates, so we decode each
page's content stream ourselves and run the PDF text-positioning state machine
(`BT`/`Tf`/`Td`/`TD`/`Tm`/`T*`/`TL` + `Tj`/`TJ`/`'`/`"`), recording each run's
baseline `(x, y)`. Runs cluster into rows (by `y`) and columns (by left-edge `x`).

- **Text-based PDFs only** — reads the embedded selectable-text layer; does not
  OCR scanned/image-only PDFs (those yield no text runs → no table).
- **Best on clean, flat tables** where each cell is drawn as one text object with
  clearly separated, left-aligned columns (typical of data/spreadsheet exports).
  Column separation is preserved over aggressive word-merging deliberately:
  over-merging two columns corrupts the table, whereas keeping runs separate is
  recoverable.
- **Complex layouts degrade** — multi-line wrapped or centered/spanning headers
  land on different `y`/`x` positions and split into extra rows/columns; cells
  drawn as multiple positioned fragments (heavy kerning / justified text) can
  over-split. This is the inherent limit of coordinate-only extraction without
  ruling-line detection; dedicated desktop tools add lattice detection + manual
  selection for these.

## Verification

- 11 core unit tests (clean 2-col×3-row table round-trips exactly to CSV/TSV/JSON,
  header mode, page selection, multi-page, RFC-4180 quoting, error paths) + 6
  block tests (schema drift-guard, arg parsing, serialize format/mime/ext).
- `wafer build` OK (lopdf instantiates under wasm32-wasip1).
- CLI end-to-end on real fetched public PDFs (W3C WAI table.pdf, dummy.pdf) for
  csv / tsv / json / header, plus graceful errors (page=0, missing url/ref,
  non-PDF content-type).

# json-to-markdown — competitor analysis (2026-07-31)

Function: render an arbitrary JSON document (object, array, or scalar) as
readable, nested Markdown — scalar keys as bullets, nested objects/arrays as
headings, uniform object arrays as GitHub tables. All findings paraphrased — no
competitor copy, branding, or trademarks reproduced.

## Top competitor tools (paraphrased)

1. **TableConvert — JSON to Markdown** (tableconvert.com) — a spreadsheet-style
   editor that ingests a JSON array (paste, `.json` upload, or scrape a table
   from a URL) and emits a Markdown table. Rich table-editing controls: undo/redo,
   transpose, dedupe rows, case conversion, find/replace. Markdown output options
   include escaping special characters, bold first row/column, per-column text
   alignment, line numbers, and a "pretty vs simple" table style. Nested cells are
   flattened to JSON strings. Browser-based, free.

2. **JSONtoTable — JSON to Markdown** (jsontotable.org) — offers three explicit
   output modes: table (array of objects → rows), list (nested bullet lists that
   preserve hierarchy), and code block (JSON wrapped in a fenced block). In table
   mode nested objects become JSON strings in cells; in list mode they become
   indented bullets. Copy-to-clipboard and `.md` download, no registration.

3. **Table.studio / MD-to / Terrific.tools** (multiple) — a cluster of single-
   purpose "JSON array → Markdown table" converters with live preview, an
   include/exclude header-row toggle, and (Table.studio) a "flatten nested JSON"
   step. All assume the input is an array of flat objects and focus on the table
   case; none render a top-level object as a heading/bullet document.

## Table-stakes / defaults / examples / UX controls

| Capability | Competitors | Our decision |
|---|---|---|
| Array of objects → Markdown table | all | **in-model** — default `auto` renders a uniform flat-object array as a GitHub table |
| Nested-object → bullet list mode | JSONtoTable, Table.studio | **in-model** — `array_mode=list` forces nested bullets; objects always render as bullets/headings |
| Force table even for ragged/nested arrays | table-first tools | **in-model** — `array_mode=table` (non-scalar cells become inline JSON) |
| Deep subtree → fenced code block | JSONtoTable (code mode) | **in-model** — subtrees past `max_depth` collapse to a fenced `json` block automatically |
| Header row present | all table tools | **in-model** — tables always emit a header + separator row |
| Column union across ragged rows | implicit | **in-model** — columns are the union of keys; missing cells blank |
| Sort keys / columns | implied by editors | **in-model** — `sort_keys` (default off = document order) |
| Heading level control | — (none render headings) | **in-model** — `heading_level` 1–6, one deeper per nesting |
| Pipe/newline escaping in cells | TableConvert escape option | **in-model** — cells single-lined and `\|`-escaped |
| Worked examples / presets | "Try it" samples | **in-model** — 3 `[[example]]` chips (note object, records→table, force list) |
| Copy / download output | all | out of scope here — the generic page shell owns copy; this repo renders unbranded |

## In-model vs out-of-model decisions

**In-model (built):** `json` (object / array / scalar), `heading_level` (1–6),
`array_mode` (auto / table / list), `sort_keys`, `max_depth` (1–20). A top-level
object becomes heading sections + `- **key**: value` bullets; a uniform flat-object
array becomes a GitHub pipe table with a unioned, optionally-sorted column set;
other arrays become nested bullet lists; subtrees deeper than `max_depth` collapse
to a fenced `json` block. Runs fully in-browser, no upload.

**Out-of-model / considered, not built:**
- **Spreadsheet-style table editing** (transpose, dedupe, case conversion,
  find/replace, undo/redo) — a bespoke interactive grid UI beyond a declarative,
  one-shot converter; belongs to an editor product, not a stateless render.
- **Per-column text alignment / bold first row / line numbers** — presentational
  table styling not expressible in portable GitHub-flavored Markdown; listed as a
  deliberate omission rather than silently dropped.
- **`.json` file upload / scrape-a-table-from-URL input** — the page takes pasted
  text; file and network ingestion are separate input models.
- **Full Markdown special-character escaping of every cell value** — we escape
  pipes and newlines (what actually breaks a table) but keep `*`/`_` verbatim so
  intentional emphasis in source strings survives; stated as a scoping choice.

# dedup-within-cell — competitor analysis (2026-09-25)

Scope: tools that remove duplicate ITEMS INSIDE a single delimited cell/list value
(`"a, b, a, c"` → `"a, b, c"`), as opposed to row/line deduplication. Findings are
paraphrased observations of publicly documented behavior — no copy, branding or assets
were reproduced.

## Competitors reviewed (top 3)

### 1. Ablebits Ultimate Suite — "Remove Duplicate Substrings" (Excel add-in)
- Closest functional match: dedupes repeated items *within* each selected cell.
- Options observed: user-specified delimiter (space, comma, comma+space, arbitrary chars);
  "treat consecutive delimiters as one" (on by default); case-sensitive vs case-insensitive
  matching.
- Keeps the FIRST occurrence of each item; documented as having no sort option, no
  keep-last option, and no count of what was removed.
- Out of model: it is a paid Excel/Windows add-in operating on a live selection.

### 2. dedup.ing — browser list deduplicator
- Deduplicates lines/rows (not within-cell), but sets the UX bar for this family.
- Options observed: case-insensitive matching, trim whitespace before comparing,
  exact match as the DEFAULT, sort output, "show counts" (input rows, unique kept,
  duplicates removed), blank handling, RFC 4180 CSV parsing, column selection in an
  advanced mode.
- Claims fully client-side processing, ~500k row capacity via Web Workers, and
  TXT/CSV/XLSX/MD/PDF export.

### 3. jsonformatterspro.com — comma separator / list cleaner
- Single-list scope (one cell's worth of items), many delimiters: comma, pipe, semicolon,
  tab, space, newline, colon, dash, underscore, slash, custom, plus auto-detect.
- Cleanup toggles: remove duplicates, trim whitespace, sort alphabetically, remove empty
  values; output quoting variants and JSON-array / SQL-IN renderings; live item counts.
- Claims browser-local processing; comfortable at 10k+ items.

Also seen (not ranked): spreadsheet formula recipes (`TEXTSPLIT`+`UNIQUE`+`TEXTJOIN` in
Excel, `SPLIT`+`UNIQUE`+`TEXTJOIN` in Sheets) — the manual baseline this tool replaces, and
the reason a table-wide, no-formula version is worth shipping.

## Gap analysis vs our build (all in-model gaps shipped in v0.1.0)

| Gap | Source | Status |
| --- | --- | --- |
| Item separator as a name or literal (comma, semicolon, pipe, space, tab, newline, `", "`, ` \| `) | 1, 3 | shipped — `item_separator` |
| Re-join with a different separator (normalize `a;b` → `a, b`) | 3 | shipped — `output_separator`, blank = reuse the cell's own spacing |
| Case-insensitive matching, exact by default | 1, 2 | shipped — `ignore_case` (default off, first occurrence's casing wins) |
| Trim whitespace around items before comparing | 2, 3 | shipped — `trim_items` (default on) |
| Treat consecutive delimiters as one / drop empty items | 1, 3 | shipped — `drop_empty` (default on) |
| Sort the surviving items | 2, 3 | shipped — `sort_items` (none/asc/desc; asc/desc are case-insensitive-aware when `ignore_case` is on) |
| Column targeting instead of the whole sheet | 1, 2 | shipped — `columns` (header names or 1-based indices; blank = every column) |
| Counts of what was removed | 2, 3 | shipped — `output = stats` (rows, cells scanned, cells changed, duplicate items removed) |
| Proper RFC 4180 quoted-field parsing | 2 | shipped — `csv` crate for both read and write |
| Non-comma CSV field delimiters (TSV, `;`, `\|`) | 2 | shipped — `delimiter` |
| Header row left untouched | 1, 2 | shipped — `has_header` (default on) |
| Browser-local, no upload, no account | 2, 3 | shipped — wasm page + CLI, nothing leaves the machine |

## Considered, not built (out of model)

- XLSX/PDF/MD export and file upload (2) — gizza pages are paste-in/text-out; CSV/TSV text is
  the exchange format here. `csv-to-xlsx` exists for the spreadsheet hop.
- Live in-spreadsheet operation on a selection (1) — needs an Office add-in host.
- Web-Worker-backed 500k-row streaming (2) — the block is a synchronous pure function; the
  1 MB input cap keeps it instant and is stated on the page.
- Auto-detect the item separator (3) — rejected on judgment, not feasibility: silently
  guessing between `,` and `;` inside a cell can corrupt data that legitimately contains
  both. The separator is explicit, with `,` as the default.
- Output quoting / JSON-array / SQL-IN renderings of a single list (3) — already covered by
  the existing `list-converter` block; duplicating it here would bloat the schema.

## Not a duplicate of an existing block

`csv-dedupe` and `csv-cleaner` remove duplicate ROWS; `find-duplicate-lines` /
`find-unique-lines` work per LINE; `list-converter` and `list-dedupe-merge` dedupe one flat
list; `csv-collapse-rows` builds delimited cells from groups of rows (the inverse direction).
None of them dedupe items inside cells that ALREADY hold delimited lists, per column, across
a table — which is this tool's job.

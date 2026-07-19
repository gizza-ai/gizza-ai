# csv-cell-diff — competitor analysis (2026-07-17)

Scan done BEFORE implementing, to set the descriptor's table-stakes. All findings are
paraphrased — no competitor copy, branding, or trademarks are reproduced. gizza runs
browser-local wasm with no server/account, so cloud/upload-only features are out-of-model.

## Competitors surveyed

1. **ExtendsClass — CSV Diff** (extendsclass.com/csv-diff.html) — paste / drag-drop / file,
   compares line-by-line and marks which fields differ.
2. **DiffCheck — CSV Diff** (diffcheck.org) — three views: Split (side-by-side), Unified
   (single panel), and a Table view for structured cell-level comparison. 100% in-browser.
3. **Datablist — CSV Diff** (datablist.com/tools/csv-diff) — treats rows as data, matches on a
   key column (auto-suggests one, override to single/multi key, or fall back to full-row
   comparison), highlights the exact cells that changed. Local-only.
4. **csvdiff.app** — row-by-row change view, cell-by-cell conflict resolution, fully client-side.
5. **CSVDiffTool** (csvdiff-tool.pages.dev) — detects added/removed rows + modified cells, choose
   a primary key, highlights changed cells inside side-by-side tables, export diff report.
6. **aifreeforever / csvkit.org / AllFileTools** — delimiter selection, color-coded add/remove/
   change markers, key-column awareness so reordered rows read as unchanged.

## Table stakes → decision

| Capability | Decision | Where |
|---|---|---|
| Cell-level diff (highlight each differing cell, show old→new) | **in-model** | core: per-cell compare, `old`/`new` in every format |
| Column alignment by header NAME (reordered columns invisible; detect added/removed columns) | **in-model** | core: intersect headers, report `added`/`removed` columns |
| Key-column row matching, single AND multiple keys | **in-model** | `key` param (comma-separated names or 1-based indices) |
| Fallback to positional / full-row comparison when no key | **in-model** | `key=""` → positional row pairing |
| Added / removed / changed / unchanged row detection + counts | **in-model** | core summary + per-row status |
| Delimiter selection (comma / tab / semicolon / pipe) | **in-model** | `delimiter` enum |
| Header toggle (first row is data) | **in-model** | `header` boolean; no-header ⇒ columns compared by position |
| Ignore case / ignore whitespace for matching | **in-model** | `ignore_case`, `ignore_whitespace` booleans |
| Multiple output views: structured report, machine-readable, exportable diff | **in-model** | `format` = `table` \| `json` \| `csv` |
| Runs locally, data never leaves the browser | **in-model** (inherent) | wasm, no network |
| Export / download the diff | **in-model** | page `format="text"` gets a Download link; `csv` format is a diff export |
| Auto-suggest the key column (heuristic) | **considered, not built** | adds guessing/ambiguity; manual `key` + positional fallback is explicit and predictable |
| Rich colored side-by-side HTML table with inline cell highlighting | **out-of-model (visual)** | this repo renders generic monospace text/JSON; a branded site repo can style the report. Core emits the structured data any UI needs |
| Drag-and-drop file upload | **out-of-model (input model)** | gizza pure tools take pasted CSV text; no file-picker for text tools |
| Cell-by-cell conflict *resolution* / merge output | **out-of-model (scope)** | this is a diff, not a merge; `blocks/csv-merge` covers concatenation, `blocks/csv-join` covers keyed joins |

## Result

Descriptor ships every in-model table stake from the start: `left`, `right`, `key` (multi-key +
positional fallback), `delimiter`, `header`, `ignore_case`, `ignore_whitespace`, `format`
(table/json/csv). Not a duplicate of `csv-merge` (stack), `csv-join` (SQL join), `json-diff`
(JSON tree), or `text-diff` (line-based) — this is the only column-aligned, cell-level CSV diff.

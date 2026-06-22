# csv-reorder-columns — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/csv-reorder-columns` — reorder, swap, or drop CSV columns by
name or index to a target order. Pure-Rust (`csv`). Pure-text input → text output:
chat + CLI + a page. Joins the existing csv-* family (filter, dedupe, pivot,
group-by, insert-column, …) with the column-ordering operation none of them cover.

## What competitors do

- **Online CSV column tools** — upload, pick columns, download. Useful but the data
  is uploaded; column ordering is often not supported (only filtering).
- **`csvcut` (csvkit) / `awk` / `cut`** — local and powerful, but require installs
  and careful flags; `cut` can't reorder columns at all (it always outputs in input
  order), a classic gotcha.
- **Spreadsheets** — drag columns manually; fine once, tedious/again-and-again and
  not scriptable.

## How this tool competes / improves

1. **Runs locally + everywhere.** Pure-Rust compiled to wasm: chat, CLI, and an
   in-browser page. The CSV never leaves the device.
2. **Reorder, drop, *and* duplicate in one step.** A target list of names (or
   1-based indices) sets the exact output order; omitted columns are dropped and a
   repeated name duplicates that column — something `cut` can't do.
3. **Name or index addressing.** Use header names (header=true) or positions; clear
   errors for an unknown name or an out-of-range index.
4. **Delimiter-flexible** (`,` / tab / `;` / `|`) and quoting-correct (real CSV
   parser, so commas inside quoted fields are handled).
5. **Same everywhere.** Identical via chat, CLI, and a `?data=…&columns=…` page.

## Honest scope

- **Column-level** reorder/drop/duplicate — not row filtering (see csv-filter),
  renaming headers, or computing new columns (see csv-formula-eval / csv-insert-column).
- Ragged rows are tolerated (missing cells become empty), matching the other csv-*
  tools.

## Tests

6 core unit tests: reorder by name; drop columns (keep a subset); reorder by index
with no header; **duplicate** a column; tab delimiter; and errors (empty input,
empty target, unknown name, out-of-range index). Plus the block drift-guard schema
test. **CLI verified** end-to-end. **Page** verified with Playwright. `wafer build`
instantiates the chat block (345 KiB).

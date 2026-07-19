# xlsx-sheet-diff — competitor analysis (2026-07-17)

Tool function: compare two worksheets of a spreadsheet cell-by-cell and report
value, formula, and structural (added/removed row & column) differences.

## Competitor scan (paraphrased — no copy/branding reproduced)

Top real tools skimmed for their function (web search, 2026-07-17):

1. **xlCompare** — desktop/online spreadsheet comparison. Advertises finding
   differences in **values, formulas, formatting, and macros**. Loads two files
   and reports differing cells. Formatting/macro comparison is its premium angle.
2. **Diffchecker (Excel compare)** — upload two spreadsheets (xlsx/xls/csv/tsv),
   get a side-by-side highlighted diff online. Broad format support; value-level
   highlighting.
3. **ExcelTool.io — Spreadsheet Compare** — free, browser-only, compares two
   Excel files side by side, highlighting row and cell differences. Emphasis on
   local (no-upload) processing.
4. **Calculations.tools — Compare Excel** — upload original + changed file, click
   "find differences"; changed cells highlighted with the previous value shown in
   parentheses. Value-diff oriented.
5. **Microsoft Spreadsheet Compare** (Office Pro Plus) — native tool; lets you
   toggle comparison categories: **Formulas, Values, Cell Formatting, Macros**.
   The canonical "choose what to compare" model.

### Key domain insight surfaced by the scan

Several tools compare **calculated values only** — if a formula changes but its
cached result is the same number, the cell reads as *unchanged*. A tool that
detects **formula-string** changes is explicitly called out as a separate need.
This is our headline differentiator: we compare **both** the displayed value and
the stored formula string, reported in separate sections.

## Table-stakes → our design

| Table-stake capability                     | In/out of model | Where it lands |
|--------------------------------------------|-----------------|----------------|
| Value (cell) differences with old→new      | in-model        | `value_changes` (changed/added/removed) |
| Formula-string differences                 | in-model        | `compare_formulas` (default on) → `formula_changes` |
| Structural diffs (rows/cols only in one)   | in-model        | `structural_changes` (extent comparison) |
| A1-style cell addressing                    | in-model        | every change is keyed by A1 (e.g. `B2`) |
| Choose what to compare                     | in-model (partial) | `compare_formulas` toggle; values always compared |
| Case / whitespace-insensitive matching     | in-model        | `ignore_case`, `ignore_whitespace` |
| Multiple output shapes                     | in-model        | `format` = table / json / csv change-log |
| Pick which two sheets/tabs to compare      | in-model        | `sheet1` / `sheet2` by name or 0-based index |
| Accept .xlsx / .xlsm / .xls / .ods          | in-model        | calamine `open_workbook_auto_from_rs` |
| **Cell-formatting diff** (fill/font/border)| **out-of-model**| calamine does not expose style deltas on read; not built |
| **Macro (VBA) diff**                        | **out-of-model**| VBA extraction/diff is a separate engine; not built |
| Two-*file* upload + side-by-side UI grid   | **out-of-model here** | see "No page surface" below |
| Colour-highlighted visual grid             | **out-of-model here** | needs a rendered grid UI (site-repo concern) |

Out-of-model items are listed, not shipped. Formatting/macro diffing would need
capabilities calamine doesn't provide on read; they are genuinely out of scope
for a pure-Rust wasm block, not merely deferred for effort.

## Design decisions

- **Two sheets within one workbook**, not two separate files. The backlog
  description ("Compares two spreadsheet **sheets** cell-by-cell") maps cleanly
  to comparing two tabs of one uploaded workbook (a real, common need: Q1 vs Q2,
  old vs new versioned tabs). This also makes the tool buildable with a single
  binary input — see below.
- **Formula comparison on by default** — the scan shows this is the capability
  competitors most often *miss*, so it is our default-on differentiator.
- **Positional cell alignment** (A1 vs A1). Row/column key-matching (reordered
  rows) is a CSV-diff feature already served by the sibling `csv-cell-diff`
  block; for two sheets of one workbook, address-aligned comparison is the
  expected mental model and keeps formula/structural reporting exact.
- **Three output formats** — `table` (human), `json` (structured, for agents),
  `csv` (flat change-log, one row per changed cell) — mirroring the sibling
  `csv-cell-diff` renderer set for cross-tool consistency.

## No page surface (correct project pattern for this input)

The generic tool-page generator renders **pure-text** field tools and **ffmpeg
media** (image/video/audio) tools. A spreadsheet is a *binary document* input
(`Input::Document` / `AssetKind::Document`) whose output here is a plain-text
diff report — this fits neither page shape. Every existing binary-document block
in this repo (e.g. `xlsx-to-csv`, `pdf-extract-text`, `docx-text-extract`) is a
**chat + CLI block with no page**, and this tool follows that established
pattern. The two locally-verifiable surfaces are the **descriptor/schema**
(drift-guarded unit test — the same schema the chat surface consumes) and the
**CLI**; both are exercised. A branded, file-upload page (or a two-file
side-by-side grid) is a private site-repo concern, out of scope for this public
toolkit.

## Verification (this run)

- `cargo test --workspace` — 11 core + 6 block tests pass (incl. schema drift guard).
- Block wasm builds (`wasm32-wasip1`) and instantiates in the CLI wasmi runtime.
- CLI end-to-end against a public multi-sheet workbook
  (`tafia/calamine/tests/issues.xlsx`): value, formula (added & removed), and
  structural changes all render; `format=csv`/`json` and `compare_formulas=false`
  exercised; single-sheet workbook returns the "need at least 2 sheets" error.

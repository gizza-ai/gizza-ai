# column-value-diff — competitor analysis (2026-07-31)

**Our tool:** Join two tables on a key column and report every row where a single chosen
*value* column disagrees, emitting clean `key → old / new` pairs. Distinct from the existing
`csv-cell-diff` (which diffs *all* columns cell-by-cell and gets noisy when the two files have
different schemas) and `csv-join` (which merges, not reconciles). This is a targeted
single-metric reconciliation — the "did the price/status/qty change for this id?" question.

Scope note: paraphrased from public product pages; no competitor copy, branding, or trademarks
reproduced. Analysis is for feature/UX ideas only.

## Competitors skimmed (top 3)

1. **Datablist CSV Diff** — browser-local CSV diff. Lets you choose a join/key column and, for
   changed rows, previews which columns changed with the old value vs the new value. Download the
   result. Broad, whole-row diff; the old/new-per-column preview is the closest analogue to our
   focused output. Key column selection, delimiter handling.

2. **CSVTool.io CSV Compare** — key-column row matching; added / removed / changed rows shown
   side-by-side with an exportable report. Colour-codes added (green), removed (red), and changed
   cells (amber). Again a whole-table diff, keyed. Exportable report is a table/CSV surface.

3. **DataflowMapper CSV Diff** — match rows by position OR by a key column (id / email) so
   reordered rows still align; shows added rows, removed rows, and the exact changed cells
   side-by-side. Emphasises "no upload, in your browser."

Other notables (not deep-read): Apify CSV Diff Tool, Pirai CSV Comparator (up to 100K rows,
client-side, field-level change tracking), csvdiff (Rust CLI), csvdiff.app.

## Feature / param inventory (paraphrased)

| Capability                          | Competitors | In our model? |
|-------------------------------------|-------------|---------------|
| Match rows by a key column          | all         | **yes** — `key` param (name or 1-based index) |
| Composite / multi-column key        | some        | **yes** — comma-separated `key` |
| Compare a single chosen value column| partial (preview per-column) | **yes — our core differentiator** (`value` param) |
| Old vs new value per change         | all (in preview) | **yes** — old/new pairs are the output |
| Alternate delimiters                | most        | **yes** — `delimiter` enum (comma/tab/semicolon/pipe) |
| Header row handling                 | all         | **yes** — `header` boolean; index fallback |
| Case-insensitive value compare      | some        | **yes** — `ignore_case` |
| Whitespace-insensitive compare      | some        | **yes** — `ignore_whitespace` |
| Include key-only-in-one-side rows   | all (added/removed) | **yes** — `include_unmatched` (report left-only / right-only for the key) |
| Structured + flat output            | export/report | **yes** — `format` = table / csv / json |
| Colour-coded side-by-side UI        | all         | out-of-model UI styling; we ship a readable table + a copyable CSV/JSON report instead |
| Cloud batch / accounts / API keys   | some (Apify) | out-of-model (server/account) — not built |
| 100K-row scale claims               | Pirai       | in-model (browser-local wasm handles large text; no artificial cap) |

## Decisions

- **Differentiator kept tight:** one `value` column, keyed join, `key → old / new` output. That is
  the reason to pick this over `csv-cell-diff` (whole-table noise) — do not grow it into a general
  cell diff.
- **In-model, built:** composite key, delimiter enum, header toggle + index fallback, case/whitespace
  folding, unmatched-key reporting, three output formats (table / csv / json).
- **Out-of-model, not built:** colour-coded side-by-side visual grid, cloud batch, accounts/API keys.
- **No competitor copy** reproduced; all page copy original.

# csv-find-incomplete-rows — competitor analysis (2026-06-22)

Tool: scan a CSV and flag rows that don't fit the file's shape — too few / too
many fields (relative to the header or first-row width), or a blank cell in a
column the caller marked as required. Returns the expected field count, the
column names, and every flagged row with its line number, field count, issue
types, and the names of any blank required columns. Pure-Rust (`csv`) → runs on
all surfaces (chat / CLI / in-browser page).

## Competitors surveyed

1. **Marty's Dev Tools — CSV Validator & Linter** (martys.app/csv-validator) —
   detects inconsistent column counts, unmatched quotes, duplicate headers,
   empty rows; per-column stats; shows the exact line number and actual-vs-
   expected column count.
2. **CSV Tools — CSV Validator** (csvtools.com/csv-validator) — ragged rows
   (inconsistent column counts), empty/duplicate header names, empty rows,
   parse warnings.
3. **SimpliConvert — CSV Validator** (simpliconvert.com/csv_validator) —
   inconsistent column counts, unclosed quotes, delimiter errors; paste or
   upload; broken-row detection.
4. **Flipper File — CSV Validator & Error Finder** (flipperfile.com) — missing
   values, invalid data types, duplicate rows, schema mismatches; runs locally
   in the browser.
5. **Online CSV Tools — Validate CSV** (onlinetools.com/csv/validate-csv) —
   ensures a CSV has no errors; finds rows/columns missing values.

## Capability diff (competitor feature → our tool)

| Feature | Competitors | csv-find-incomplete-rows |
| --- | --- | --- |
| Ragged-row detection (wrong field count) | all | yes — `too_few_fields` / `too_many_fields`, distinguished |
| Exact line number per bad row | Marty's, others | yes — `line` (true source line via the csv reader's position, quote-span-aware) |
| Actual vs expected field count | Marty's | yes — `expected_fields` + per-row `fields` |
| Blank / missing required cells | Online CSV Tools, Flipper | yes — `required` by column **name or 1-based index**; whitespace-only counts as blank |
| Configurable delimiter | most | yes — `,` / tab / `;` / `|` or any single char |
| Header vs headerless | most | yes — `header` toggle; headerless uses first-row width + `col1…` names |
| Data row index (excluding header) | — | yes — `row` (1-based among data rows), in addition to `line` |
| Returns raw cell values for context | some | yes — `values` per flagged row |
| Runs locally / no upload | Flipper, all browser tools | yes — page is pure in-browser wasm; nothing uploaded |
| Structured machine-readable output | some | yes — chat/CLI return JSON; page renders a readable line-per-row summary |

## Gaps vs competitors (and disposition)

- **Per-column type validation (e.g. "column 3 must be an integer/date/email").**
  Out of this tool's scope — that is schema validation, better served by a
  dedicated `csv-schema-validate` tool. The existing `csv-stats` already infers
  numeric-vs-text per column. Not added here to keep this tool focused on
  structural completeness.
- **Duplicate-header / duplicate-row detection.** Adjacent concerns covered by
  other gizza blocks (`csv-dedupe` for duplicate rows). Left out to avoid
  overlap; this tool is strictly about row completeness/shape.
- **Unclosed-quote diagnostics as a distinct issue.** The `csv` reader surfaces
  a malformed-quote parse as an error (returned verbatim), rather than a
  per-row flag; this matches the underlying RFC 4180 parser's behaviour.

## Verification (this run)

- `cargo test --workspace` in `blocks/csv-find-incomplete-rows` — 14 core/block
  tests pass, including the drift-guard schema test.
- `wafer build` — chat block validates and instantiates (332 KiB).
- CLI (`gizza tool csv-find-incomplete-rows …`) — verified ragged rows, blank
  required column, and a clean CSV (no flags).
- Page (Playwright, xvfb) — 2 specs pass: ragged-row flags and blank-required
  flag render in-browser.

No competitor copy, branding, or trademarks were used.

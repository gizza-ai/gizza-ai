# numeric-string-sanitizer — competitor analysis (2026-08-06)

Scan run before completing the partial scaffold. Findings are paraphrased; no competitor copy, branding, or trademarks are reused.

## Function under study

Take messy text cells that humans recognize as numbers — currency symbols, thousands separators, units, percent signs, accounting parentheses, whitespace, non-breaking spaces, and suffixes such as K/M/B — and emit plain floats suitable for CSV imports, spreadsheets, analytics, or model features.

## Duplicate / viability check

Checked `blocks/` for numeric, CSV, format, currency, percent, and string-cleanup tools. Existing calculators and CSV tools either compute on already-valid numbers, validate column types, or format text; none accepts a pasted numeric column and normalizes each messy cell into a float. This is pure Rust string parsing and fits the gizza model.

## Competitors reviewed

### 1. Spreadsheet VALUE/NUMBERVALUE style functions

- Table-stakes: explicit decimal and group separator conventions, not just one locale.
- Useful behavior: parse currency-formatted strings and percent-like strings after cleanup.
- Gap for web tooling: formulas operate one cell at a time and do not provide an audit of failed rows.
- In-model decisions: `decimal_separator=auto|dot|comma`, one output row per input row, and an `on_error` policy.

### 2. Data-cleaning libraries / notebook workflows

- Common pattern: remove non-numeric characters with regex, replace locale separators, then parse as floats.
- Table-stakes: handle thousands separators, whitespace, accounting parentheses, and optional rounding.
- Gap: regex snippets often silently mangle ambiguous comma/dot conventions or percent values.
- In-model decisions: column-level decimal inference, `parentheses_negative`, explicit percent behavior, row-status output.

### 3. Online CSV/data cleaning utilities

- Common controls: paste data, transform a column, choose output format, inspect failed values.
- Table-stakes UX: multiline textarea, presets/examples, TSV/JSON audit output, summary counts.
- Out-of-model features: uploaded multi-column CSV editor, persistent projects, batch files, and charting dashboards.
- In-model decisions: textarea input, preset chips, values/table/json formats, optional summary statistics.

## Gap list → decisions

| Capability | Fit | Decision |
| --- | --- | --- |
| Currency symbol and unit stripping | in-model | Built with core parser that extracts the numeric body and ignores surrounding text. |
| Thousands separators including spaces/apostrophes/NBSP | in-model | Built. |
| Dot vs comma decimal conventions | in-model | Built: auto inference plus explicit dot/comma override. |
| Accounting parentheses and trailing minus | in-model | Built: default on, checkbox control. |
| Percent handling | in-model | Built: strip or divide by 100. |
| K/M/B/T suffix expansion | in-model | Built: default on, ordinary units do not scale. |
| Error policies | in-model | Built: blank, keep, marker, fail. |
| Audit output | in-model | Built: values, TSV table, JSON. |
| Summary stats | in-model | Built: optional count/sum/min/max/average. |
| Multi-column CSV editor | out-of-model for this tool | Not built; use CSV tools when row/column structure must be preserved. |
| ML-based unit inference | out-of-model | Not built; suffix expansion is deterministic and conservative. |

## Copy / UX notes taken into the page

- Make row alignment and error policy obvious, because failed cells are the main danger in cleaning columns.
- Explain percent stripping vs division clearly.
- State the 20,000-row cap and the single-convention limitation of auto decimal detection.
- Provide examples for currency cleanup, percent fractions, and audit tables.

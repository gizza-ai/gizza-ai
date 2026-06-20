# csv-dedupe — competitor analysis (2026-06-20)

Twenty-sixth `/create-next-tool` backlog pick. Pure-Rust (`csv` crate) text tool,
all 3 surfaces. Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| online "remove duplicate rows" CSV tools | dedupe full rows or by selected columns, keep first, in-browser | capabilities |
| spreadsheet dedupe | choose key columns, case sensitivity, keep first/last | capabilities |

## Gap diff vs our tool
Our tool: remove duplicate rows (first kept); key on the whole row by default, or
a subset of `columns` (1-based indices, or header names when header=true);
configurable delimiter; header row preserved. Covers the core dedup feature set.

**In-model gaps considered, deferred (minor):**
- **Case-insensitive / trim-insensitive** key matching — a `case_sensitive`
  toggle.
- **Keep last** instead of first.
- **Report count** of removed rows alongside the output.

**Out-of-model:** fuzzy/near-duplicate matching (needs similarity scoring — a
heavier, separate tool).

## Tested
unit (5: full-row dedupe keeps header, keyed on column name, keyed on index w/o
header, no-header full row, errors for empty/unknown-name/0-index/name-without-
header) + drift-guard · wafer fixtures (1) · `wafer build` · wasm-pack web ·
generator · CLI (keyed on 'name' keeps first Alice) · Playwright page + query
deep-link (2).

> Original work only — no competitor copy, branding, or trademarks copied.

# csv-insert-column — competitor analysis (2026-06-20)

Thirty-first `/create-next-tool` backlog pick. Pure-Rust (`csv` crate) text tool,
all 3 surfaces. Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| online "add column to CSV" tools | new column with a constant, position choice, in-browser | capabilities |
| spreadsheet insert-column | position, fill value, header | capabilities |

## Gap diff vs our tool
Our tool: insert a new column at a 1-based position (or append with 'end'), filled
with a constant value; the header row gets the column name; position clamped to the
row width; configurable delimiter. Covers the core insert-a-constant-column.

**In-model gaps considered, deferred (the row mentions "or a per-row template"):**
- **Per-row template** — e.g. `{a}-{b}` interpolating other columns into the new
  cell. A nice extension; the purely-arithmetic version is already covered by the
  csv-formula-eval tool (computed columns), so this would add a string-template
  mode here. Documented as a follow-up.
- **Insert by reference to a column name** ("after column X") instead of an index.

**Out-of-model:** none notable.

## Tested
unit (5: append by default, insert at front, insert in middle, no-header fills
every row, errors for empty/invalid-position) + drift-guard · wafer fixtures (1) ·
`wafer build` · wasm-pack web · generator · CLI (insert at position 1) · Playwright
page + query deep-link (2).

> Original work only — no competitor copy, branding, or trademarks copied.

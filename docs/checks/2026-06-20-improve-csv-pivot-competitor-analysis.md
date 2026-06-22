# csv-pivot — competitor analysis (2026-06-20)

Thirty-third `/create-next-tool` backlog pick. Pure-Rust (`csv` crate) text tool,
all 3 surfaces. Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| online "CSV pivot table" tools | rows/columns/values + aggregate, cross-tab, in-browser | capabilities |
| spreadsheet pivot | multi-level rows/cols, totals, multiple value fields | capabilities |

## Gap diff vs our tool
Our tool: rows (one+ columns) × a columns-field (its distinct values spread across
the top) × an aggregated values column (sum/count/avg/min/max). First-seen order
for rows + columns; empty cells blank. Covers the core single-value pivot.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **Row/column grand totals** (a totals row/column).
- **Multiple value fields** (e.g. sum of sales AND count) → multiple cell columns.
- **Multi-level column headers** (pivot on >1 column field).
- **Sort columns** (e.g. alphabetical instead of first-seen).

**Out-of-model:** interactive drag-drop pivot UI (the page is a form; the pivot is
specified by params).

## Tested
unit (3: basic sum pivot with a missing cell blank, count pivot, errors for empty/
unknown-column/bad-agg/no-header) + drift-guard · wafer fixtures (1) · `wafer build`
· wasm-pack web · generator · CLI (region×product sum: N A=13 B=5, S A=7) ·
Playwright page (sum pivot).

> Original work only — no competitor copy, branding, or trademarks copied.

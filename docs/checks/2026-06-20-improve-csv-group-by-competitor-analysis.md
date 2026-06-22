# csv-group-by — competitor analysis (2026-06-20)

Twenty-ninth `/create-next-tool` backlog pick. Pure-Rust (`csv` crate) text tool,
all 3 surfaces. Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| online CSV "group & aggregate" tools | group by columns, count/sum/avg/min/max, in-browser | capabilities |
| pandas-groupby / SQL GROUP BY web tools | multi-key grouping, many aggregates, having/sort | capabilities |

## Gap diff vs our tool
Our tool: group by one or more columns (names or 1-based indices); aggregate with
count/sum/avg/min/max via a `column:func` list (+ bare `count`); one output row
per group in first-seen order; numeric aggregates ignore non-numeric cells.
Covers the core group-and-aggregate feature set.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **More aggregates** — median, stddev, first/last, distinct-count, concat.
- **Sort output** by a group or aggregate column (pairs with a csv-sort tool).
- **HAVING-style filter** on the aggregated result (chain with csv-filter).

**Out-of-model:** full pivot/cross-tab (that's csv-pivot — values spread across
columns); windowed/rolling aggregates.

## Tested
unit (4: sum + count per group, avg/min/max, multi-column group key, errors for
empty/unknown-column/empty-aggs/bad-func/no-header) + drift-guard · wafer fixtures
(1) · `wafer build` · wasm-pack web · generator · CLI (dept → sum/avg/count) ·
Playwright page + query deep-link (2).

> Original work only — no competitor copy, branding, or trademarks copied.

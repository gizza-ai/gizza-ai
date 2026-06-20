# csv-filter — competitor analysis (2026-06-20)

Twenty-seventh `/create-next-tool` backlog pick. Pure-Rust (`csv` crate) text
tool, all 3 surfaces. Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| online "filter CSV" / CSV-query tools | filter rows by a column condition; numeric + text ops; in-browser | capabilities |
| SQL-over-CSV tools | full WHERE clauses, multiple conditions, ORDER BY | capabilities |

## Gap diff vs our tool
Our tool: keep rows where `<column> <op> <value>` holds; ops == != < <= > >= and a
case-insensitive `contains`; column by header name or 1-based index; numeric
compare when both sides are numbers else string; spaces optional (`age>=30`);
header preserved. Covers the core single-condition filter.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **Multiple conditions (AND/OR)** — the single most-requested extension; would
  grow the expression grammar (kept deliberately small + well-tested for v1).
  Documented on the page as "run twice / chain tools" for now.
- **starts_with / ends_with / regex** operators.
- **Negation / invert** (keep non-matching rows).

**Out-of-model:** full SQL engine over CSV (a much larger tool); ORDER BY/GROUP BY
(separate sort/aggregate tools).

## Tested
unit (6: numeric > , string ==, contains case-insensitive, no-space condition +
index without header, !=, errors for empty/unknown-column/no-operator/missing-
column) + drift-guard · wafer fixtures (1) · `wafer build` · wasm-pack web ·
generator · CLI (numeric >= and contains) · Playwright page + query deep-link (2).

> Original work only — no competitor copy, branding, or trademarks copied.

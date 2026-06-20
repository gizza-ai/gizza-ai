# csv-formula-eval — competitor analysis (2026-06-20)

Twenty-eighth `/create-next-tool` backlog pick. Pure-Rust (`csv` + `meval`) text
tool, all 3 surfaces. Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| spreadsheet / CSV "add calculated column" tools | formula over columns, add/replace column, in-browser | capabilities |
| SQL-over-CSV / pandas-eval tools | rich expressions, functions, multiple new columns | capabilities |

## Gap diff vs our tool
Our tool: `<column> = <expression>` formulas (`;`/newline separated, run
left-to-right), referencing columns by header name; arithmetic + `^` + parens +
meval's function library (sqrt/abs/min/max/round/floor/ceil/trig/ln/…); a new
name appends a column, an existing name replaces it; non-numeric cells blank a
referencing formula for that row. Covers the core add/transform-columns feature
with chained formulas.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **String/text functions** (concat, upper, substring) — meval is numeric-only;
  text formulas would need a different/extended engine.
- **Conditionals** (if/ternary) — meval has no `if`; a small extension or a
  different expr crate could add it.
- **Column names with spaces** — referenced names must be identifiers; a `[Col
  Name]` quoting syntax (mapped to safe vars internally) is a future add.
- **Aggregates** (sum over a column) — that's csv-pivot/csv-group-by territory.

**Out-of-model:** full spreadsheet engine (cell refs A1, ranges), which is a much
larger scope.

## Tested
unit (6: adds computed column, transforms existing column, chained formulas see
earlier results, math function sqrt, non-numeric cell → blank, errors for empty/
no-`=`/invalid-expr/missing-target) + drift-guard · wafer fixtures (1) ·
`wafer build` · wasm-pack web · generator · CLI (chained total then taxed) ·
Playwright page + query deep-link (2).

> Original work only — no competitor copy, branding, or trademarks copied.

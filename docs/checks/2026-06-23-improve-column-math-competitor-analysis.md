# column-math — competitor analysis (2026-06-23)

New tool built from the backlog and reviewed against the common web-based
"column / list math" and "vector calculator" tools. All competitor notes are
paraphrased — no copy, branding, or trademarks were reproduced.

## What it does

Element-wise arithmetic between two equal-length numeric columns: `add` (A+B,
default), `subtract` (A−B), `multiply` (A×B), `divide` (A÷B). Columns are pasted
as comma-, space-, or newline-separated numbers; the result is returned one value
per row.

## Surfaces verified

- **Chat / API:** `cargo test --workspace` (10 core + 1 drift-guard schema test,
  all pass) + `wafer build` validates `target/block.wasm` (314 KiB) + `wafer
  test` runs 6 invoke fixtures (add / subtract / multiply / divide / length
  mismatch / divide-by-zero) — 6 passed.
- **CLI:** `gizza tool column-math a=… b=… operation=…` across all four
  operations, the default-add path, newline-separated input, and the two error
  paths (length mismatch → exit 1; divide-by-zero → exit 1, names the row).
- **Page:** Playwright (4 specs) — default add, divide via the `<select>`, the
  length-mismatch error message, and an `?a=…&b=…&operation=multiply`
  query-param deep-link.

## Competitors surveyed

1. **Generic "add two lists of numbers" online tools** — paste two lists, pick an
   operation, get the element-wise result. Most accept newline or comma input;
   some only support a single operation (add) per page.
2. **Vector calculator sites (e.g. matrix/vector arithmetic pages)** — element-
   wise add/subtract of equal-length vectors, plus dot product and scalar
   multiply; usually rigid comma input and a fixed dimension.
3. **Spreadsheet apps (Excel / Google Sheets)** — the reference behaviour: a
   fill-down `=A1+B1` formula across two columns. The benchmark for "what column
   math should do", but requires opening a spreadsheet.
4. **"List/column operations" utility pages** — sort, dedupe, sum a single
   column; a few offer two-column element-wise ops as one mode among many.
5. **Programmer REPL snippets (numpy `a + b`, JS `map`)** — the power-user path;
   exact element-wise semantics but needs a runtime and code.

## Gap diff vs our build

| Gap | Dimension | Decision |
| --- | --- | --- |
| All four element-wise operations (+ − × ÷) | capability | **Shipped** — many single-purpose pages only add |
| Flexible input separators (comma / space / newline, mixed) | capability | **Shipped** — paste straight from a spreadsheet column (one per line) or a CSV row |
| Clear, row-numbered error on length mismatch / divide-by-zero | copy/UX | **Shipped** — names how many values each column has, and the offending row |
| Decimals + negatives, whole results without trailing `.0` | capability | **Shipped** |
| Query-param deep-link (`?a=…&b=…&operation=…`) | UX | **Shipped** via the page driver |
| Scalar operand (apply one number to a whole column) | capability | **Considered, not built** — element-wise two-column math is the stated scope; a scalar can be entered by repeating the value, and a dedicated scale tool would be a cleaner home |
| Reductions (sum / mean / dot product of the result) | capability | **Out of scope** — this tool is element-wise → element-wise; aggregation belongs to a separate stats/sum tool |
| Live two-column grid UI / spreadsheet-style cells | UX/visual | **Out of model** — the shared page driver renders text fields + a single output area, not an editable grid |
| Mixed-length broadcasting (numpy-style) | capability | **Out of scope** — silently broadcasting is error-prone for a general-audience tool; we require equal length and say so explicitly |
| Power / modulo operations | capability | **Considered, not built** — add/subtract/multiply/divide are the four operations the backlog spec names ("add, subtract, multiply, or divide"); kept tight to spec |

## Conclusion

The initial build already covers the in-model capability set (all four
operations, flexible input, robust errors, decimals/negatives, deep-link), so no
additional capability was added in an improvement pass — the gaps that remain are
either out of the shared page driver's model (editable grid) or deliberately out
of scope (reductions, broadcasting, extra operators) to keep the tool focused and
its errors predictable. The competitive edge here is the forgiving multi-separator
input (paste a spreadsheet column directly) plus precise, row-numbered error
reporting that most single-operation list-math pages lack.

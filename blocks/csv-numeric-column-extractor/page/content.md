## About this tool

Spreadsheets and database dumps mix identifiers, labels, dates and measurements in one table. This
tool reads a CSV or TSV and tells you which columns are **actually numeric** — then hands those
columns back as typed arrays you can drop straight into a chart, a statistics routine or a JSON
payload.

It sniffs the **delimiter** (comma, tab, semicolon or pipe), decides whether the **first row is a
header**, and tests every cell in every column. A column qualifies when all of its non-missing
values parse as numbers; loosen that with **Minimum numeric share** if a stray label shouldn't
disqualify an otherwise numeric column. Each qualifying column reports whether it is an `integer`
or a `float`, how many values it holds and how many were missing — and every rejected column comes
back in a `skipped` list with the reason and an offending example, so you never have to guess why a
column disappeared. Everything runs locally in your browser; the data is never uploaded.

### Worked example

Given this input:

```
id,name,score
1,Alice,9.5
2,Bob,7
```

the default **Typed arrays** output is:

```json
{
  "delimiter": "comma",
  "header": true,
  "rows": 2,
  "columns_total": 3,
  "numeric_columns": 2,
  "columns": [
    {
      "name": "id",
      "index": 1,
      "type": "integer",
      "count": 2,
      "missing": 0,
      "numeric_ratio": 1,
      "values": [1, 2]
    },
    {
      "name": "score",
      "index": 3,
      "type": "float",
      "count": 2,
      "missing": 0,
      "numeric_ratio": 1,
      "values": [9.5, 7]
    }
  ],
  "skipped": [
    {
      "name": "name",
      "index": 2,
      "reason": "only 0 of 2 value(s) parse as numbers (0% < the 100% required)",
      "example": "Alice",
      "numeric_ratio": 0
    }
  ]
}
```

Switch **Output** to *Column names only* for `id`, then `score` on the next line; to *Numeric
columns as CSV* for `id,score` / `1,9.5` / `2,7`; or to *Row objects* for
`[{ "id": 1, "score": 9.5 }, { "id": 2, "score": 7 }]`.

### Accounting formats

With **Accept $1,234.50 / 45% / (500) formatting** left on, exported figures still count as
numbers: thousands separators (`1,234.50` → `1234.5`), currency symbols `$ € £ ¥ ₹`, a trailing
percent sign (`45%` → `45`), parentheses negatives (`(500)` → `-500`) and trailing minus
(`250-` → `-250`). Turn it off when you want strictly plain numbers.

### Limits & edge cases

- Input is capped at **1 MB** of pasted text (about 1,000,000 characters). Split larger exports and
  run the parts separately.
- Zero-padded codes such as `007` or a ZIP like `01234` are **not** treated as numbers, so
  identifier columns don't get silently converted.
- `inf`, `Infinity` and `NaN` are treated as text, not numbers.
- Missing cells are the empty cell plus anything in **Null tokens** (matched exactly and
  case-sensitively after trimming). They become `null` in the output and are excluded from the
  numeric share.
- Ragged rows are padded: a row shorter than the widest row counts as missing in the trailing
  columns. Wholly blank lines are dropped.
- Values are re-emitted as parsed numbers, so `1,234.50` comes back as `1234.5` and trailing zeros
  are not preserved.

## FAQ

<details>
<summary>What makes a column count as "numeric"?</summary>

Every non-missing cell in it has to parse as a number. Set **Minimum numeric share** below `1` to
relax that — at `0.75`, a column where three of four values are numbers still qualifies, and the
value that didn't parse is emitted as `null`.

</details>

<details>
<summary>My column has a few blanks — will it still be extracted?</summary>

Yes. **Allow blank cells in a numeric column** is on by default, so gaps become `null` and the
column is still returned; the `missing` count tells you how many there were. Turn it off to require
a value in every row.

</details>

<details>
<summary>Why was my ID or ZIP column skipped?</summary>

Zero-padded values like `007` or `01234` are deliberately rejected: they are identifiers whose
leading zeros would be lost as numbers. The `skipped` list names the column and shows the value
that disqualified it.

</details>

<details>
<summary>How does it choose the delimiter and the header row?</summary>

With **Delimiter** on *Auto-detect* it parses the text once per candidate (comma, tab, semicolon,
pipe) and keeps whichever yields the most consistent column count, requiring at least two columns.
With **Header row** on *Auto-detect*, the first row is treated as a header unless one of its cells
is itself a number — so `10;20;30` on line one is read as data and the columns are named
`column_1`, `column_2`, `column_3`. Both can be forced.

</details>

<details>
<summary>Can I get the result as CSV instead of JSON?</summary>

Yes — set **Output** to *Numeric columns as CSV* for a comma-separated table containing only the
numeric columns (missing values become empty cells), or to *Column names only* for a plain list you
can paste into a query or a script.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely in your browser tab — the CSV you paste
never leaves your machine.

</details>

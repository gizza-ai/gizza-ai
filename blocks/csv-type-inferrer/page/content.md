## About this tool

Paste a chunk of CSV and this tool figures out its structure for you. It sniffs the
**delimiter** (comma, semicolon, tab or pipe) by testing which one splits the rows into the most
consistent number of columns, decides whether the **first row is a header**, and then infers a
**type for every column** — `int`, `float`, `bool`, `date`, `datetime`, `string`, or `empty`. The
result is a JSON schema report plus the rows re-emitted as type-coerced JSON records. Everything
runs locally in your browser; the CSV is never uploaded.

### Worked example

Given this input:

```
id,name,active,joined,score
1,Alice,true,2021-05-01,9.5
2,Bob,false,2022-11-30,7
```

the schema report includes:

```json
{
  "dialect": { "delimiter": ",", "delimiter_name": "comma", "quote_char": "\"", "has_header": true },
  "row_count": 2,
  "column_count": 5,
  "columns": [
    { "name": "id",     "type": "int",    "nullable": false, "non_empty": 2, "missing": 0, "unique": 2 },
    { "name": "name",   "type": "string", "nullable": false, "non_empty": 2, "missing": 0, "unique": 2 },
    { "name": "active", "type": "bool",   "nullable": false, "non_empty": 2, "missing": 0, "unique": 2 },
    { "name": "joined", "type": "date",   "format": "YYYY-MM-DD", "nullable": false, "non_empty": 2, "missing": 0, "unique": 2 },
    { "name": "score",  "type": "float",  "nullable": false, "non_empty": 2, "missing": 0, "unique": 2 }
  ]
}
```

Choose **Records only** to get just the typed rows, or **Schema only** to skip the records.

### Limits & edge cases

- Zero-padded integers such as `007` or ZIP codes like `01234` are kept as **strings** so leading
  zeros survive.
- A column is only given a numeric/date/bool type when **every** non-null value in it matches;
  one stray value keeps the whole column a string.
- Null tokens are matched **exactly and case-sensitively** against the trimmed cell (plus the
  empty string is always null).
- Dates are validated (month/day ranges, leap years). Supported date shapes are `YYYY-MM-DD`,
  `YYYY/MM/DD`, `MM/DD/YYYY`, `DD/MM/YYYY` and the dash variants; datetimes are `YYYY-MM-DD`
  with a `T` or space separator and an optional trailing `Z`.

## FAQ

<details>
<summary>How does it pick the delimiter?</summary>

With **Delimiter** set to *Auto-detect*, it parses the text once with each candidate (comma,
semicolon, tab, pipe) and keeps the one that produces the most consistent column count across
rows, requiring at least two columns. Set a specific delimiter to skip sniffing.

</details>

<details>
<summary>How does header detection work?</summary>

In *Auto-detect* mode it compares the first row against the inferred type of each column's data
rows: if the data is clearly typed (numbers, dates, booleans) but the first row's cell is text, it
votes that the first row is a header. An all-text table can't be told apart, so it assumes a header
is present. Force the behaviour with **Header row → First row is a header** or **No header**.

</details>

<details>
<summary>Why is my number column showing up as a string?</summary>

Every value in the column has to parse for the column to take that type. A single non-numeric
cell, a stray label, a thousands separator like `1,000`, or a leading-zero code such as `007` will
keep the column as `string`. Add unusual missing-value markers to **Null tokens** so they don't
block inference.

</details>

<details>
<summary>What counts as a missing value?</summary>

An empty cell is always treated as missing, plus any token you list in **Null tokens**
(default `NA,N/A,NULL,null,None,nan`). Missing cells become `null` in the records and are excluded
from type inference; the `missing` count and `nullable` flag in the schema reflect them.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely in your browser tab — the CSV you paste
never leaves your machine.

</details>

## About this tool

**Query Result Formatter** turns the raw rows you get back from a database query
or an API — a **JSON** array, **CSV**, **TSV**, or a **`psql`/MySQL/SQLite CLI
table** — into a clean, aligned table you can paste straight into a README, an
issue, a PR, or chat.

- **Auto-detect input:** paste rows as-is. A leading `[` or `{` is read as JSON, a
  `+---+`/`---+---` rule as a SQL shell table, a tab in the first line as TSV,
  otherwise CSV — or pick the format explicitly.
- **Reads database shell output directly:** paste a `psql`, `mysql`, or `sqlite3`
  result table and the pipe columns, `+---+` rules, and the `(2 rows)` footer are
  cleaned up for you.
- **JSON, the way query results actually look:** an array of row objects becomes a
  table whose columns are the union of every row's keys (in first-seen order);
  a key missing from a row is filled with your **null text**. Arrays of arrays,
  a single row object, and arrays of scalars all work too.
- **Markdown or ASCII:** `markdown` emits a GitHub-style pipe table (with `|` and
  newlines escaped so it stays valid); `ascii` emits a box-drawing `+---+` grid for
  logs, comments, or plain-text docs.
- **Alignment + nulls:** choose left, right, or center column alignment, and set the
  text that a JSON `null` or a missing value renders as (e.g. `NULL`).

### Worked example

Input (JSON query result):

```
[{"id":1,"name":"Ada"},{"id":2,"name":"Linus"}]
```

Output (Markdown):

```
| id  | name  |
| --- | ----- |
| 1   | Ada   |
| 2   | Linus |
```

### Privacy

Everything runs **in your browser** via WebAssembly — your data is never uploaded.
Also available from the gizza CLI and in chat.

### Common uses

- Turn a `psql`/BigQuery TSV dump into a Markdown table for a report or ticket.
- Format a JSON API response as a readable table without hand-aligning columns.
- Drop a spreadsheet selection (tab-separated) into Markdown or a plain-text grid.

## FAQ

<details>
<summary>What input formats can I paste?</summary>

A JSON array of row objects, a JSON array of arrays, a single JSON object, a JSON
array of scalars, CSV, or TSV. Leave the input format on **Auto** and the tool
sniffs the shape from the first characters, or choose `json`, `csv`, or `tsv`
explicitly if the auto guess is wrong.

</details>

<details>
<summary>Can I paste output straight from psql, mysql, or sqlite3?</summary>

Yes. Choose **SQL CLI table** (or leave the input on Auto — a `+---+` or
`---+---` rule gives it away). The border pipes, the horizontal rules, and a
trailing `(N rows)` / `rows in set` footer are stripped, and the first row is used
as the header. An empty psql cell is treated as a null and renders as your
null / missing text.

</details>

<details>
<summary>How are JSON objects with different keys handled?</summary>

The columns are the **union** of every row's keys, in the order they first appear.
A row that is missing one of those keys shows your **null / missing text** in that
cell (empty by default; set it to `NULL` to mark database nulls). Nested arrays and
objects are kept as compact JSON so a cell never breaks the table.

</details>

<details>
<summary>What is the difference between Markdown and ASCII output?</summary>

**Markdown** produces a GitHub-flavored pipe table (`| a | b |`) with a `|---|---|`
separator row — ready to paste into Markdown files, issues, or PRs; `|` and line
breaks inside cells are escaped so it stays valid. **ASCII** produces a
box-drawing grid with `+---+` borders, ideal for log output, code comments, or
plain-text documents where Markdown isn't rendered.

</details>

<details>
<summary>My CSV has no header row — will the first row of data be lost?</summary>

No. Turn the **First row is header** toggle off and the tool generates `Column 1`,
`Column 2`, … headers, and every row (including the first) lands in the table body.
JSON arrays-of-objects always use the object keys as headers regardless of this
toggle.

</details>

<details>
<summary>What if my rows have different numbers of fields?</summary>

The table is sized to the widest row and shorter rows are padded with empty cells,
so ragged CSV/TSV or uneven JSON arrays still produce a rectangular table instead of
an error.

</details>

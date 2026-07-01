## CSV query

Query a CSV with a small SQL-style language — projection, filtering, sorting, and
limiting — all in your browser. Nothing is uploaded.

### Syntax

```
SELECT <columns | *> [WHERE <col op value>] [ORDER BY <col> [ASC|DESC]] [LIMIT n]
```

- **Columns** — header names or 1-based indices, comma-separated; `*` for all.
- **WHERE** — one condition: op is `==`, `!=`, `<`, `<=`, `>`, `>=`, or
  `contains` (case-insensitive). Numeric comparison when both sides are numbers,
  else string.
- **ORDER BY** — sort by a column, `ASC` (default) or `DESC`; numeric-aware.
- **LIMIT** — keep the first N rows.

### Examples

- `SELECT name, age WHERE age >= 30 ORDER BY age DESC`
- `SELECT * WHERE city contains ny LIMIT 10`

For grouping/aggregation use the CSV group-by or pivot tools.

### FAQ

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly.

</details>

<details>
<summary>Can I combine conditions with AND / OR?</summary>

Not yet — `WHERE` takes exactly one `<column> <op> <value>` condition. To
apply two filters, run the query once, then feed the result back in with the
second condition. For grouping or aggregation, use the CSV group-by or pivot
tools instead.

</details>

<details>
<summary>My file uses semicolons or tabs, not commas — will it work?</summary>

Yes. Set the delimiter to the actual separator character, or use one of the
names `comma`, `tab`, `semicolon`, or `pipe`. Anything longer than a single
character (other than those names) is rejected.

</details>

<details>
<summary>Why is my first row of data missing from the results?</summary>

The first row is always read as the header — it supplies the column names that
`SELECT`, `WHERE`, and `ORDER BY` refer to. If your CSV has no header, add one,
or refer to columns by 1-based index (`SELECT 1, 3`).

</details>

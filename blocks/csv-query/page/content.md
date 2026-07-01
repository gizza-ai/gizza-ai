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

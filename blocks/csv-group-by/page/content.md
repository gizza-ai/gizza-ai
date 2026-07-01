## CSV group by

Summarize a CSV by grouping rows on one or more columns and aggregating the rest.
For example, group by `dept` and compute `amount:sum` and a row `count`. It runs
in your browser; nothing is uploaded.

### Options

- **Group by** — one or more columns (header names or 1-based indices,
  comma-separated).
- **Aggregations** — a list of `column:func` where func is `count`, `sum`, `avg`,
  `min`, or `max`; or a bare `count` for the per-group row count. Non-numeric
  cells are ignored by the numeric aggregates.
- Output has one row per group (in first-seen order): the group columns followed
  by the aggregated columns (named like `sum_amount`).

### FAQ

<details>
<summary>How are non-numeric or empty cells aggregated?</summary>

`count` counts every row in the group regardless of content. The numeric
aggregates parse each cell as a number and skip anything that doesn't parse:
`avg` divides by the number of *numeric* cells only, and `min`/`max` come out
empty for a group with no numeric values at all (`sum` reports `0`).

</details>

<details>
<summary>Can I group by several columns, or by column number?</summary>

Yes on both counts. **Group by** takes a comma-separated list, and each entry
can be a header name (`region,dept`) or a 1-based index (`1,3`) — handy when
headers contain commas or you'd rather count columns. Multi-column groups
produce one output row per distinct combination.

</details>

<details>
<summary>What delimiters are supported, and what order are the rows in?</summary>

Comma (default), tab, semicolon, pipe — by name or as the literal character —
or any other single-character separator. The output uses the same delimiter.
Groups appear in **first-seen order**, i.e. the order their first row appears
in the input, not sorted.

</details>

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly.

</details>

<details>
<summary>Need a cross-tab/pivot?</summary>

Use the CSV pivot tool.

</details>

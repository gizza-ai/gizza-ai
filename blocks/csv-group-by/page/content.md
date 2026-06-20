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

**Is my data uploaded?** No — it's processed locally with WebAssembly.

**Need a cross-tab/pivot?** Use the CSV pivot tool.

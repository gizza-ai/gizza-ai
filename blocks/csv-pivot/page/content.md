## CSV pivot table

Summarize a CSV as a pivot table (cross-tab). Pick the **row** column(s), the
**column** field whose distinct values spread across the top, and the **value**
column to aggregate in each cell. It runs in your browser; nothing is uploaded.

### Options

- **Row column(s)** — comma-separated names/indices; their distinct combinations
  are the output rows.
- **Column field** — its distinct values become the output columns.
- **Value column** — aggregated per (row, column) cell.
- **Aggregation** — `sum` (default), `count`, `avg`, `min`, or `max`. Empty cells
  (no matching rows) are left blank.

### Example

`region, product, sales` pivoted with rows=`region`, columns=`product`,
values=`sales`, agg=`sum` → one row per region, one column per product, totals in
the cells.

### FAQ

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly.

</details>

<details>
<summary>Can I group by more than one row column?</summary>

Yes. Give the row field a comma-separated list — column names or 1-based indices,
e.g. `region,product` or `1,3`. Each distinct combination of those values becomes
one output row.

</details>

<details>
<summary>My CSV uses semicolons or tabs — will it work?</summary>

Yes. Set the delimiter option to the actual separator: a single character, or one
of the words `comma`, `tab`, `semicolon`, `pipe`. The default is a comma, and the
first row must always be a header.

</details>

<details>
<summary>What order are the pivot rows and columns in, and what goes in empty cells?</summary>

Both row keys and pivot columns appear in first-seen order from your data — they
are not sorted alphabetically. A cell with no matching source rows is left blank
rather than showing 0, so "no data" stays distinguishable from "sums to zero".

</details>

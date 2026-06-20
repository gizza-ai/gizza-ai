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

**Is my data uploaded?** No — it's processed locally with WebAssembly.

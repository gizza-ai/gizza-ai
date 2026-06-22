## CSV insert column

Add a new column to a CSV, filled with a constant value, at any position. For
example, append a `source = import` column, or insert an `id` column at the front.
It runs in your browser; nothing is uploaded.

### Options

- **New column name** — the header for the column (used when *first row is a
  header* is on).
- **Fill value** — the constant written into every data row.
- **Position** — a 1-based index, or `end` to append. Clamped to the row width.
- **Delimiter** — comma, tab, semicolon, pipe, or any single character.

### FAQ

**Is my data uploaded?** No — it's processed locally with WebAssembly.

**Want per-row computed values instead of a constant?** Use the CSV formula tool.

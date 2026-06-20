## CSV dedupe

Remove duplicate rows from a CSV, keeping the **first** occurrence. By default a
row counts as a duplicate only when the whole row matches; give one or more **key
columns** to dedupe on just those (e.g. keep the first row per `email`). It runs
in your browser; nothing is uploaded.

### Options

- **Key columns** — names (when there's a header) or 1-based indices, comma-
  separated. Blank means match the entire row.
- **First row is a header** — keep it and allow naming columns.
- **Delimiter** — comma, tab, semicolon, pipe, or any single character.

### FAQ

**Which duplicate is kept?** The first one in order; later duplicates are dropped.

**Is my data uploaded?** No — it's processed locally with WebAssembly.

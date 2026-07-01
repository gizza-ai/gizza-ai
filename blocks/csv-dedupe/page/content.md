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

<details>
<summary>Which duplicate is kept?</summary>

The first one in order; later duplicates are dropped.

</details>

<details>
<summary>Can I dedupe on more than one column?</summary>

Yes — list the key columns comma-separated, as header names (`email,company`)
or 1-based indices (`1,3`). A row only counts as a duplicate when **all** key
columns match an earlier row; mixing names and indices in one list works too.

</details>

<details>
<summary>Is the duplicate check case-sensitive?</summary>

Yes. Values are compared exactly as they appear, so `Alice` and `alice` are
different, and a stray leading space makes a row unique. Normalize the data
first (trim whitespace, lower-case a key column) if you need fuzzy matching.
Rows with fewer fields than the key columns treat the missing fields as empty.

</details>

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly.

</details>

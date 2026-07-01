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

<details>
<summary>What happens if the position is larger than the number of columns?</summary>

It's clamped: asking for position 10 on a 3-column CSV simply appends the new column at the end (same as `end`). Position is 1-based, so `1` inserts at the very front.

</details>

<details>
<summary>What if my rows have different lengths?</summary>

Ragged CSVs are accepted — rows aren't forced to a uniform width. The insert position is clamped per row, so a short row still gets the new value at its own end rather than causing an error.

</details>

<details>
<summary>How do I use a tab or semicolon as the separator?</summary>

Type the word `tab`, `comma`, `semicolon`, or `pipe` in the delimiter field — or any single character (e.g. `;`). Multi-character delimiters other than those names are rejected. The output uses the same delimiter as the input.

</details>

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly.

</details>

<details>
<summary>Want per-row computed values instead of a constant?</summary>

Use the CSV formula tool.

</details>

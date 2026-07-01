## About this tool

**CSV stats** gives you a quick `describe()`-style summary of a CSV: for each
column you get the value **count**, **empty-cell** count, and number of **distinct**
values — and for columns whose values are all numbers, the **min**, **max**,
**mean**, and **sum**.

Paste a CSV (with or without a header) and pick the delimiter (`,` / tab / `;` /
`|`). Great for sizing up a dataset before you analyse it.

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded.
Also available from the [gizza CLI](/) and in chat (which return the stats as
structured JSON).

### Common uses

- Spot the range and average of a numeric column at a glance.
- Find columns with missing (empty) cells.
- Check how many distinct values a category column has.

## FAQ

<details>
<summary>Why doesn't my column show min / max / mean / sum?</summary>

Those are only computed when **every non-empty value** in the column parses as
a number. A single stray value like `N/A`, `12,5` (comma decimal) or a
currency symbol flips the whole column to *text*, which reports only count,
distinct and empty. Empty cells are fine — they're skipped, not counted as
non-numeric.

</details>

<details>
<summary>What happens if my CSV has no header row?</summary>

Untick "first row is a header" and the tool names the columns `col1`, `col2`, …
in order. If you leave the header option on, the first row is used for names
(blank header cells also fall back to `colN`) and is excluded from the stats.

</details>

<details>
<summary>Which delimiters are supported?</summary>

Comma (default), tab, semicolon and pipe by name, or any other **single
character** typed directly into the delimiter field. Rows with different
field counts are accepted; shorter rows simply contribute empty cells to the
trailing columns.

</details>

<details>
<summary>Is my CSV uploaded anywhere?</summary>

No — the whole summary is computed in your browser with WebAssembly, so the
data never leaves your device.

</details>

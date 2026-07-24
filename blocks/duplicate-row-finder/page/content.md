## Find duplicate rows in a CSV or table

Paste a CSV (or any delimited table) and this tool finds the rows that repeat,
groups them together with their line numbers, and tells you **which columns are
driving the duplication**. It is read-only: it *reports* the duplicates, it does
not delete or rewrite anything. Everything runs in your browser — nothing is
uploaded.

By default two rows match only when the **whole row** is identical. Give one or
more **key columns** to match on just those (e.g. treat rows with the same
`email` as duplicates even when the name differs). Turn on **Ignore letter case**
and **Ignore extra whitespace** to catch near-duplicates that differ only by
casing or stray spaces.

### Worked example

Input (with the header row on):

```
name,email
Alice,a@x.com
Bob,b@y.com
Alice,a@x.com
Carol,c@z.com
Bob,b@y.com
```

Report output:

```
Scanned 5 data rows (2 columns), keyed on the whole row.
Found 2 duplicate groups covering 4 rows; 1 row is unique.

Duplicate groups (most repeated first):
  x2  Alice | a@x.com   (lines 2, 4)
  x2  Bob | b@y.com   (lines 3, 6)

Columns driving duplication (repeated rows / distinct values):
  name: 4 / 3
  email: 4 / 3
```

Line numbers count from the top of the input (the header is line 1), so you can
jump straight to each duplicate in your source file.

### Options

- **Key columns** — header names (when there's a header) or 1-based indices,
  comma-separated. Blank matches the entire row.
- **First row is a header** — use it to name columns and exclude it from the scan.
- **Delimiter** — comma, tab, semicolon, pipe, or any single character.
- **Ignore letter case / extra whitespace** — normalize before comparing so
  near-duplicates that differ only by case or spacing still group together.
- **Output** — *Report* (a readable summary plus the column analysis),
  *Duplicate rows only (CSV)* (just the offending rows, ready to paste back), or
  *Structured (JSON)* (groups + per-column stats for scripting).

### Limits & edge cases

- Comparisons are exact after the case/whitespace normalization you choose — this
  catches formatting differences, not typos. For typo-level fuzzy matching use the
  fuzzy-dedupe tool instead.
- Blank cells are ignored in the "columns driving duplication" analysis (an empty
  value is not a meaningful duplicate), but they still count as part of a whole-row
  or key match.
- Rows with fewer fields than expected treat the missing cells as empty.
- Everything is processed in memory in your browser; very large files are bounded
  by your device's memory.

### FAQ

<details>
<summary>Does it remove the duplicates?</summary>

No — this tool only *finds and reports* them. Choose the **Duplicate rows only
(CSV)** output to export the offending rows, or use the csv-dedupe / fuzzy-dedupe
tools when you want the cleaned data back.

</details>

<details>
<summary>What does "columns driving duplication" mean?</summary>

For every column it counts how many rows carry a value that also appears in
another row (`repeated rows / distinct values`). A column near the top of that
list is where the repetition is concentrated — for example a repeated `email`
while every `id` stays unique. It helps you pick the right key column to match on.

</details>

<details>
<summary>Can I match on more than one column?</summary>

Yes. List the key columns comma-separated, as header names (`email,company`) or
1-based indices (`1,3`); mixing names and indices works too. A row counts as a
duplicate only when **all** the key columns match an earlier row.

</details>

<details>
<summary>How are near-duplicates handled?</summary>

With **Ignore letter case** and **Ignore extra whitespace** on (both default on),
`Alice` matches `alice` and `New  York ` matches `New York`. That covers casing
and spacing differences; it does not merge misspellings — for that, try the
fuzzy-dedupe tool.

</details>

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly. Nothing leaves your browser.

</details>

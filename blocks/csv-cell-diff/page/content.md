# CSV Cell Diff

Compare two CSV files column-by-column and see exactly which **individual cells**
changed — not just which lines differ. Unlike a plain text diff, this tool understands
column structure: it lines columns up by header name (so reordered columns still match)
and can pair rows by a key column (so reordered rows still match), then reports every
changed cell with its old and new value, plus which rows and whole columns were added or
removed. Everything runs locally in WebAssembly — your data never leaves the browser.

## Worked example

**Original CSV**

```
id,name,price
1,Apple,10
2,Banana,20
3,Cherry,30
```

**Updated CSV**

```
id,name,price
1,Apple,12
2,Banana,20
4,Date,15
```

With **Key column(s)** = `id` and **Output format** = `table`:

```
3 columns compared
1 rows changed · 1 rows added · 1 rows removed · 1 rows unchanged · 1 cells changed
~ [1] price: "10" → "12"
- [3] id=3, name=Cherry, price=30
+ [4] id=4, name=Date, price=15
```

Row `1`'s price cell changed 10 → 12, row `4` is new, and row `3` was removed. Row `2` is
unchanged. Switch **Output format** to `json` for a structured report (with per-cell
`old`/`new` and summary counts) or `csv` for a flat `row_key,status,column,old,new`
change-log you can pipe into another tool.

The same example through the `gizza` CLI:

```
gizza tool csv-cell-diff left=$'id,name,price\n1,Apple,10\n2,Banana,20\n3,Cherry,30' right=$'id,name,price\n1,Apple,12\n2,Banana,20\n4,Date,15' key=id format=table
```

## Inputs

- **Original CSV** / **Updated CSV** — the two versions to compare.
- **Key column(s)** — one or more column names (or 1-based indices), comma-separated, used
  to pair rows so reordering doesn't register as a change. Leave empty to pair rows by
  position (row 1 vs row 1, and so on).
- **Delimiter** — comma, tab, semicolon, or pipe (applied to both CSVs).
- **First row is a header** — on by default; columns are then aligned by name. Turn it off
  to align columns by position (`col1`, `col2`, …).
- **Ignore case** / **Ignore whitespace** — fold case or collapse whitespace when
  comparing, while the output still shows the original text.
- **Output format** — a readable table report, a structured JSON report, or a flat CSV
  change-log.

## Limits & notes

- Both inputs are pasted text; there is no file upload. Very large CSVs are limited by
  browser memory.
- Comparison is per-cell exact string match (after any case/whitespace folding); numbers
  like `10` and `10.0` are treated as different text.
- With a key column, duplicate key values are paired in order of appearance; any leftover
  rows on either side are reported as added or removed.
- A whole column that exists on only one side is listed as added/removed and its cells are
  not compared against the other file.

## FAQ

<details>
<summary>How is this different from a plain text diff?</summary>

A text diff compares whole lines, so moving a row or reordering columns shows up as a big
change. This tool parses the CSV structure: it aligns columns by header name and (with a
key column) matches rows by identity, then reports the specific cells that differ. The
result points at `row → column: old → new`, not just "this line changed".

</details>

<details>
<summary>What does the key column do, and can I use more than one?</summary>

The key column(s) identify each row so the tool can match the same record across both
files even if the rows were reordered. Give a single column like `id`, or several
comma-separated columns like `first,last` for a composite key. Leave it empty to compare
rows positionally (row 1 vs row 1). Columns can be referenced by header name or by 1-based
index.

</details>

<details>
<summary>My two files have the columns in a different order — is that a problem?</summary>

No. When the first row is treated as a header, columns are aligned by **name**, so a file
with `price,id,name` compares correctly against one with `id,name,price`. Only columns
that exist in both files are compared cell-by-cell; a column present on just one side is
reported as added or removed.

</details>

<details>
<summary>When should I pick the JSON or CSV output instead of the table?</summary>

Use **table** for a quick, readable review. Use **json** when a script needs the
structured result — it includes the common/added/removed columns, summary counts, and a
per-row list of changed cells with `old` and `new` values. Use **csv** for a flat
`row_key,status,column,old,new` change-log that you can open in a spreadsheet or feed into
another CSV tool.

</details>

<details>
<summary>Does my data get uploaded anywhere?</summary>

No. The comparison runs entirely in your browser via WebAssembly. Nothing is sent to a
server, so it is safe to diff private or sensitive exports.

</details>

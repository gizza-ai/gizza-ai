## About this tool

**Constant Column Dropper** finds the **zero-variance columns** in a CSV or
delimited table — the ones that carry a single repeated value down every data
row, or that contain nothing at all — and removes them.

Constancy is measured as *one distinct value*, not as a statistical variance of
zero. That matters: the distinct-value rule works on text columns
(`country = US` in every row) as well as numbers, and it handles blank cells
without special-casing. Columns like this add no information to a model, a
pivot, or a report — they just make the table wider.

Set **Dominance** below 100 to also catch *near*-constant columns: at 95, a
column whose most common value covers 95% or more of the rows is flagged too.
Everything runs locally in your browser — your data is never uploaded.

### Worked example

Input:

```
id,country,score,notes
1,US,10,
2,US,20,
3,US,30,
4,US,40,
```

Default report:

```
Scanned 4 columns across 4 data rows (dominance 100%).
Found 2 constant columns; 2 columns remain.

Constant columns (dropped):
  "country" (col 2)  =  "US" in 4/4 rows (100%)
  "notes" (col 4)  =  all cells are empty

Use output=csv to get the table with those columns removed.
```

With **Output = Cleaned CSV**, the result is:

```
id,score
1,10
2,20
3,30
4,40
```

**JSON metrics** returns the same verdict per column plus the numbers behind it —
`distinct_values`, `top_value`, `top_count` and `top_share_percent` — so you can
script the decision instead of eyeballing it.

### Options

- **First row is a header** — on by default. The header row is excluded from the
  value counts and preserved in cleaned CSV output.
- **Delimiter** — comma, tab, semicolon, or pipe. Output uses the same one.
- **Dominance threshold** — 100 (default) drops only strictly constant columns.
  Lower it to drop near-constant ones: 95 means "at least 95% of the rows say the
  same thing". The range is 50–100.
- **Empty cells** — *Count as a value* (default) means a column of values plus
  blanks is not constant; *Skip when counting* ignores blanks first, so a column
  that is `gold`, blank, `gold` counts as constant. A column that is entirely
  empty is dropped either way.
- **Ignore case** — on by default, so a column of `YES` / `yes` counts as constant.
- **Ignore whitespace** — on by default, so `US` and ` US ` are the same value.
- **Never drop these columns** — comma-separated column names or 1-based column
  numbers to protect. A protected column stays in the output and is listed
  separately in the report.
- **Output** — human report, cleaned CSV, or JSON metrics.

### Limits and edge cases

- The last column standing is never removed silently: if *every* column would be
  dropped, **Cleaned CSV** returns an error naming the count instead of emitting
  an empty table. Switch to the report to see what happened, or protect a column.
- A column that is entirely empty is always dropped, whichever empty-cell mode is
  selected — there is no value in it to keep.
- Case and whitespace normalization affect the *comparison* only. Cells are
  written to the cleaned CSV exactly as you pasted them.
- Ragged rows are allowed: the table is as wide as its widest row, and missing
  cells are counted as empty.
- A header row alone, with no data rows, is an error — there is nothing to
  measure.
- This is a paste-sized page. Multi-hundred-megabyte files belong in a data
  pipeline, not a browser tab.

## FAQ

<details>
<summary>What exactly counts as a constant column?</summary>

A column whose data rows hold exactly one distinct value after the optional case
and whitespace normalization — or a column with no non-empty cells at all. That
is the same rule as `nunique() == 1` in a dataframe, or `min == max`, and unlike
a statistical variance test it works on text columns too.

</details>

<details>
<summary>How do I catch columns that are almost constant?</summary>

Lower the **Dominance threshold**. At 95, a column is dropped when its most
frequent value covers 95% or more of the counted rows — the "near-zero-variance"
case that feature-selection tools flag alongside true constants. The report
always shows the actual share, so you can see how close a call it was.

</details>

<details>
<summary>Does a blank cell count as a value?</summary>

Your choice. With **Empty cells = Count as a value** (the default) a column of
`gold`, blank, `gold` has two distinct values and survives. With **Skip when
counting**, blanks are removed first and the column reads as constant. Dataframe
tools differ on this, which is why it is a switch rather than a fixed rule.

</details>

<details>
<summary>Can I protect an ID or label column from being dropped?</summary>

Yes. List it under **Never drop these columns**, by header name or by 1-based
column number (`id, 3`). Protected columns stay in the cleaned CSV and are
reported separately, so you still learn that they were constant.

</details>

<details>
<summary>Is my table uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely in your browser. Your
CSV/table data never leaves your device.

</details>

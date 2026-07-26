## About this tool

Collapse rows groups a CSV by one or more **key columns** and joins another
column's values from every row in the group into a single delimited cell. It is
the spreadsheet-and-file equivalent of SQL `GROUP_CONCAT` / `STRING_AGG` /
`LISTAGG`, pandas `groupby(keys)[col].agg(", ".join)`, and R dplyr
`summarise(paste(x, collapse = ", "))`.

Point it at the columns to group by and the column to collapse, and it returns
**one row per group** — the key columns followed by the collapsed column — in
first-seen order. You can dedupe repeated values (like `DISTINCT`), sort them
ascending or descending, skip blank cells, choose the separator between values,
and switch the field delimiter to tab, semicolon, or pipe. Everything runs
locally in your browser; the CSV never leaves your machine.

### Worked example

Input:

```
region,product
East,Apple
West,Banana
East,Cherry
East,Apple
```

With **group-by** `region`, **collapse** `product`, and **dedupe** on:

```
region,product
East,"Apple, Cherry"
West,Banana
```

The two `East` rows merge into one; the repeated `Apple` is dropped by dedupe,
and the cell is quoted because it contains the comma separator.

## Limits & edge cases

- Columns can be referenced by **header name** or **1-based index** (`1` = first
  column). Turn off *First row is a header* to treat every row as data — then use
  indices only, and output columns are labelled `col_1`, `col_2`, ….
- Output keeps only the **key columns and the collapse column**; any other
  columns are dropped.
- Groups and values within a cell are emitted in **first-seen order** unless you
  choose a sort. Sorting is lexicographic (text), so `10` sorts before `2`.
- **Skip blank cells** (on by default) drops empty values before joining; turn it
  off to keep empty positions (e.g. `a,,b`).
- The value separator can be any string (default `, `). The field delimiter is
  separate and applies to both input parsing and output.

## FAQ

<details>
<summary>How is this different from a group-by that sums or counts?</summary>

This tool concatenates the text values of one column into a list; it does not
compute numeric aggregates like sum, average, or count. If you need those, use a
CSV group-by / aggregate tool instead. Collapse rows is for turning many rows of
labels, tags, or IDs into one comma-separated cell per group.

</details>

<details>
<summary>Can I group by more than one column?</summary>

Yes. Enter a comma-separated list in **Group-by column(s)** — for example
`year,team`. Rows are grouped by the exact combination of those key values, and
both key columns are carried through to the output.

</details>

<details>
<summary>Why is a collapsed cell wrapped in double quotes?</summary>

Standard CSV quoting. When the joined value contains the field delimiter (a comma
by default), a line break, or a quote, the cell is wrapped in double quotes so the
result re-parses correctly. Choosing a different separator or field delimiter can
avoid the quoting if you prefer.

</details>

<details>
<summary>Does the order of the collapsed values match my input?</summary>

By default, yes — values appear in the order they were first seen, matching SQL
`GROUP_CONCAT` without an `ORDER BY`. Choose **Ascending** or **Descending** under
*Sort collapsed values* to order them alphabetically instead. Dedupe also keeps
the first occurrence of each value.

</details>

<details>
<summary>My file has no header row — can I still use it?</summary>

Yes. Turn off **First row is a header**. Then every row is treated as data, you
reference columns by 1-based index (`1`, `2`, …), and the output gets generated
column labels `col_1`, `col_2`, … so the result is still a valid CSV.

</details>

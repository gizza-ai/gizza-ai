## About this CSV window-function tool

Window functions compute a value across a set of rows that are related to the
current row, **without collapsing** those rows the way `GROUP BY` does. This tool
brings the SQL `OVER (PARTITION BY … ORDER BY …)` idea to a plain CSV: every input
row is kept, and one new column is appended with the window result.

Pick a function, choose the value column by header name (for example `sales`) or
1-based index (for example `3`), and optionally partition and order the rows:

```csv
region,day,sales
W,2,5
E,1,10
W,1,3
E,2,20
```

A **running total** of `sales`, partitioned by `region` and ordered by `day`,
produces one cumulative column, with rows grouped by partition:

```csv
region,day,sales,running_total_sales
W,1,3,3
W,2,5,8
E,1,10,10
E,2,20,30
```

### The functions

- **running_total** — cumulative sum of the value column within each partition.
- **moving_average** — trailing mean over the last `window` rows (including the
  current row); early rows average whatever rows exist so far.
- **lag** / **lead** — the value `offset` rows before (lag) or after (lead) the
  current row; rows with no neighbour are left blank.
- **rank** — position by value within the partition. Tied values share a rank and
  the next rank skips (1, 1, 3, 4).
- **dense_rank** — like rank, but with no gaps after ties (1, 1, 2, 3).
- **row_number** — a 1-based counter within each partition; needs no value column.

### Partitions and order

`partition_by` splits the rows into independent windows — one per distinct
combination of the listed columns — so a running total restarts for each region
and a rank is computed group by group. `order_by` sorts rows within each partition
before the function runs, which is what makes a running total or lag meaningful.
Turn on **Descending** to reverse the sort and to rank the largest value first.

Numbers are detected automatically; blank or non-numeric cells are skipped by the
numeric functions (running total, moving average) rather than treated as zero.

Everything runs locally in your browser. Your CSV data is never uploaded.

### FAQ

<details>
<summary>What is the difference between rank and dense_rank?</summary>

Both give tied values the same rank. `rank` then leaves a gap — two values tied
for 1st are followed by 3rd (1, 1, 3, 4). `dense_rank` never leaves gaps, so the
next value is 2nd (1, 1, 2, 3). Use `row_number` when you want every row to get a
distinct number even on ties.

</details>

<details>
<summary>How does the moving average handle the first few rows?</summary>

The window is trailing and includes the current row, so with `window=3` the first
row averages just itself, the second averages the first two, and from the third
row on it averages the current row plus the two before it. Non-numeric cells in the
frame are ignored, so a blank value does not drag the average toward zero.

</details>

<details>
<summary>Do I need to sort my CSV first?</summary>

No. Set `order_by` to the column that defines the sequence (such as a date or day
number) and the tool sorts each partition for you before computing. Leave
`order_by` empty to use the rows in their existing order. The output rows are
grouped by partition and follow the order you chose.

</details>

<details>
<summary>Can I compute a window per group, like per customer or per region?</summary>

Yes. Put the grouping column(s) in `partition_by` — for example `region`, or
`region,category` for a combined key. Each distinct combination becomes its own
window, so running totals restart and ranks are computed within each group
independently.

</details>

<details>
<summary>Why does the tool require a header row?</summary>

Columns are selected by header name (with 1-based index as a fallback), and the
appended column needs a header too. Keep the first row as your header. You can
still reference columns by number, such as `3`, if your headers are not
descriptive.

</details>

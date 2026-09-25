## About this tool

Use this when a CSV column contains a list packed into each cell — tags, SKUs, email recipients, categories, audiences — and the same item appears more than once inside that cell. The table shape stays the same: no rows are removed and no unrelated columns are changed. Only the list inside each selected cell is cleaned.

Worked example: with `data` set to:

```csv
id,tags
1,"a, b, a, c"
2,"red, blue, red"
```

set `columns = tags`, leave `item_separator = comma`, and keep first-seen order. The result is:

```csv
id,tags
1,"a, b, c"
2,"red, blue"
```

Leave **Columns to clean** blank to process every cell, or give a comma-separated list of header names and 1-based column numbers such as `tags,4`. Turn on **Ignore case** to treat `Apple`, `apple` and `APPLE` as the same item while keeping the first spelling. Choose **Sort A-Z** or **Sort Z-A** when the cleaned list should be deterministic instead of first-seen.

Limits and edge cases: input is capped at 1 MB. The parser is CSV-aware, so quoted cells with commas and embedded newlines are handled correctly. `has_header` keeps the first row untouched and lets you use header names; turn it off for raw one-column lists. `output_separator` can normalize joins (for example `, `) or stay blank to preserve each cell's original separator spacing.

## FAQ

<details>
<summary>Is this the same as deleting duplicate rows?</summary>

No. Row dedupe removes repeated records. This tool keeps every row and only removes repeated **items inside a cell**, such as turning `marketing, sales, marketing` into `marketing, sales`.

</details>

<details>
<summary>Can I target just one column?</summary>

Yes. If the input has a header row, put the header name in `columns`, such as `tags`. You can also use 1-based indices like `2`, or mix them: `tags,4`. Leave it blank to process every column.

</details>

<details>
<summary>How are spaces handled?</summary>

`trim_items` is on by default, so `a, b, a` compares as `a`, `b`, `a` and writes the trimmed values back. If `output_separator` is blank, the first separator style seen in that cell is reused, so comma-space cells stay comma-space.

</details>

<details>
<summary>What happens to empty items?</summary>

`drop_empty` is on by default, so repeated separators such as `a,,b,,,a` collapse to `a,b`. Turn it off if blank list positions carry meaning in your data.

</details>

<details>
<summary>Can it handle TSV or semicolon-separated files?</summary>

Yes. Use `delimiter` for the table delimiter (`tab`, `semicolon`, `pipe`, or `comma`) and `item_separator` for the separator inside a cell. Those are separate controls because a TSV cell can still contain comma-separated tags.

</details>

<details>
<summary>Will the order of items change?</summary>

Not by default. `sort_items = none` keeps the first occurrence of each item in the order it appeared. Pick `asc` or `desc` only when you want each cell's items sorted.

</details>

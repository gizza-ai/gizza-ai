## Count category values in one CSV column

Paste a CSV or delimited table, choose a single column by header name or 1-based
index, and this tool returns a compact `value,count,percent` CSV. It is the
browser-local version of the pandas `Series.value_counts()` workflow: count each
distinct value, compute its share of the counted rows, and sort the result by
frequency or by value.

### Worked example

Input:

```csv
fruit
apple
banana
apple
cherry
apple
banana
```

With **Column = fruit**, the output is:

```csv
value,count,percent
apple,3,50%
banana,2,33.33%
cherry,1,16.67%
```

Use **Case-sensitive grouping** off to group `Open`, `open`, and `OPEN` together
while keeping the first spelling. Turn **Include blank cells** on to count blank
cells as `(empty)` instead of skipping them.

### Limits & edge cases

- The first row is always treated as the header. Pick a column by exact header
  name or by 1-based position (`1`, `2`, ...).
- Blank cells are skipped by default. When included, whitespace-only cells count
  as `(empty)`.
- Percentages use the total counted values as the denominator and are rounded to
  at most two decimals.
- Ties in **Sort by count** keep the first-seen order from the input column.
- CSV quoting is handled by the Rust `csv` parser/writer, so commas inside quoted
  fields are preserved.

## FAQ

<details>
<summary>How is this different from count-line-frequency?</summary>

`count-line-frequency` treats every input line as one value. This tool parses a
CSV/table first and counts only the column you choose, then adds a percentage
column and delimiter-aware output.

</details>

<details>
<summary>Can I count a tab- or semicolon-delimited file?</summary>

Yes. Set **Delimiter** to `tab`, `semicolon`, `pipe`, `comma`, or any single
character. The output uses the same delimiter so it can be pasted back into a
spreadsheet.

</details>

<details>
<summary>What happens to empty cells?</summary>

They are skipped by default, matching the common "drop missing values" workflow.
Enable **Include blank cells** to count every blank or whitespace-only cell as the
literal value `(empty)`.

</details>

<details>
<summary>How are percentages calculated?</summary>

Each percentage is `count / total counted values * 100`. If blanks are skipped,
they are not part of the denominator; if blanks are included, `(empty)` counts
like any other value.

</details>

<details>
<summary>Is my CSV uploaded?</summary>

No. Parsing and counting happen in WebAssembly in your browser; the pasted table
never leaves your machine.

</details>

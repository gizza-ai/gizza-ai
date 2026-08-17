## About this tool

Sorted Order Checker verifies numeric sequences without changing the data. Paste comma-separated, newline-separated, space-separated, tab-separated, semicolon-separated, or pipe-separated values and the tool reports whether the list is sorted ascending, descending, or in the direction it auto-detects from the first differing pair.

The report is designed for debugging spreadsheets, generated IDs, timestamps, ranking exports, sensor readings, and batch jobs where one misplaced value can break a pipeline. When the list is not sorted, it names the first out-of-order element with its 1-based position, value, previous neighbour, and the comparison that failed. It also reports the total number of broken neighbour comparisons and the longest already-sorted run.

### Worked example

Input:

```text
1, 4, 9, 2, 7
```

With `order=auto`, `strict=false`, and `separator=auto`, the report starts:

```text
Not sorted ascending (equal neighbours allowed) — the order first breaks at position 4.
First out-of-order element: position 4, value 2 (previous position 3, value 9)
```

Use `strict=true` when repeated values should fail, such as a unique timestamp or sequence-number check. Use `strip_thousands=true` with an explicit separator (for example `space`) when values contain digit-group separators like `1,024 2,048 4,096`.

### Limits and edge cases

- Accepts up to 20,000 numeric values per run.
- Numbers may be integers, decimals, negatives, leading `+`, or scientific notation such as `1e6`.
- Blank entries are skipped.
- `NaN` and non-numeric tokens are errors by default; set `non_numeric=ignore` to skip header words or placeholders and report how many were ignored.
- `auto` direction treats an all-equal list as constant when ties are allowed; with `strict=true`, repeated neighbours are reported as breaks.
- `max_issues` controls how many broken comparisons are listed; totals still count every break.

## FAQ

<details>
<summary>What does “auto” order do?</summary>

`auto` looks for the first pair of values that are different. If the second is larger, the list is checked as ascending; if it is smaller, the list is checked as descending. This is useful when you only care whether the list is monotonic in either direction.

</details>

<details>
<summary>How is strict mode different from the default?</summary>

By default, equal neighbours are allowed, so `1, 2, 2, 5` is sorted ascending. With `strict=true`, every step must change in the chosen direction, so the repeated `2` is reported as an out-of-order value.

</details>

<details>
<summary>Can I paste values with thousands separators?</summary>

Yes. Set `strip_thousands=true` and choose a separator that is not a comma, such as `space` or `newline`. For example, `1,024 2,048 4,096` with `separator=space` parses as three values.

</details>

<details>
<summary>What happens if my list has headers or `n/a` entries?</summary>

The default `non_numeric=error` stops and names the first bad token so you can fix the data. If the labels are expected, set `non_numeric=ignore`; the tool skips those tokens, checks the numeric values that remain, and reports how many were ignored.

</details>

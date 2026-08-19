## About this tool

Use this first difference calculator when you have a series of ordered numbers and want the row-to-row movement, a seasonal lag, or second differences for a quick linear/quadratic check. Paste values separated by spaces, commas, semicolons, or newlines; the tool keeps the output aligned to the original row numbers by default so you can trace every result back to its source row.

Worked example: for `120, 135, 150, 148, 161` with `lag = 1`, `order = 1`, and `mode = difference`, the aligned values are `[null, 15, 15, -2, 13]`. The first value is `null` because there is no previous row to compare against. Switch to `mode = percent` and `drop_warmup = true` to get signed period-over-period percent changes such as `12.5`, `11.111111`, `-1.333333`, and `8.783784`.

Common settings:

- `lag = 1` compares each value with the value immediately before it.
- `lag = 12` compares monthly data with the same month one year earlier.
- `order = 2` computes second differences, useful for spotting constant quadratic-style growth.
- `mode = percent` returns signed percent change, while `mode = ratio` returns the raw growth factor.
- `drop_warmup = true` returns the shorter R-style output instead of aligned `null` warm-up rows.

Limits and edge cases: the input must contain 2 to 20,000 finite numbers. `lag` can be negative for lead-style comparisons, but it cannot be zero. A zero baseline in percent or ratio mode, or a non-positive value in log mode, is returned as `null` and counted in `summary.undefined` rather than as infinity.

## FAQ

<details>
<summary>What is a first difference?</summary>

A first difference subtracts the previous value from the current value. For `2, 5, 9, 14`, the aligned first differences are `null, 3, 4, 5` because the first row has no previous value.

</details>

<details>
<summary>How is percent mode different from ratio mode?</summary>

Percent mode returns `(current - baseline) / baseline × 100`, so `100 → 110` is `10`. Ratio mode returns `current / baseline`, so the same move is `1.1`. Both are directional period-over-period measures, not the symmetric two-number percent difference used by some calculators.

</details>

<details>
<summary>Why does the output contain null values?</summary>

`null` means either there is no baseline row for that position, or the comparison is mathematically undefined. Keep the default aligned output when row numbers matter; enable “Drop the warm-up rows” when you want the shorter list of computed values only.

</details>

<details>
<summary>Can I compute seasonal or second differences?</summary>

Yes. Use a larger `lag` for seasonal differencing, such as `12` for monthly data. Use `order = 2` for second differences; constant second differences are a quick signal of a quadratic pattern when the input rows are evenly spaced and ordered.

</details>

## Normalize the numeric columns of a CSV

Paste a CSV and this tool feature-scales its numeric columns locally in your
browser — **nothing is uploaded**. Pick the normalization method that fits your
data:

- **Min-max** — linearly rescale each column into a target range (default `0`–`1`)
  using its own minimum and maximum. Good for bounding features to a fixed
  interval.
- **Z-score** — standardize each column to mean `0` and unit standard deviation.
  Choose population (`ddof = 0`) or sample (`ddof = 1`, Bessel-corrected) stddev.
- **Robust** — center on the median and scale by the interquartile range (IQR,
  the 25th–75th percentile spread). Resistant to outliers; toggle centering and
  scaling independently.

Only **numeric** columns are scaled — a column counts as numeric when every
present value parses as a finite number. Text columns are copied through
unchanged, and **blank cells stay blank** (they're excluded from the column's
statistics).

### Worked example

With the default **Min-max** method scaling to `0`–`1`, this input:

```
name,height,weight
Alice,150,50
Bob,160,60
Carol,170,70
```

becomes:

```
name,height,weight
Alice,0,0
Bob,0.5,0.5
Carol,1,1
```

`height` runs `150`–`170`, so `160` sits halfway → `0.5`; `weight` runs `50`–`70`,
so `60` → `0.5`. The `name` column is text, so it's copied through untouched.

### FAQ

<details>
<summary>What is the difference between min-max, z-score, and robust scaling?</summary>

**Min-max** rescales a column into a fixed interval (like `0`–`1`) based on its
minimum and maximum, so outliers can compress the rest of the values. **Z-score**
(standardization) shifts each column to mean `0` and divides by its standard
deviation, giving unbounded values centered on zero. **Robust** scaling uses the
median and interquartile range instead of the mean and stddev, so a few extreme
values barely move the result — pick it when your data has outliers.

</details>

<details>
<summary>Which columns get normalized?</summary>

By default every **numeric** column. Type a comma-separated list under **Columns
to scale** to restrict it — use header names (like `height,weight`) when the first
row is a header, or 1-based indexes (like `2,3`). Columns you don't list, and any
text columns, are copied through unchanged.

</details>

<details>
<summary>How are blank cells and text columns handled?</summary>

A blank (empty or whitespace-only) cell stays blank and is left out of the column's
minimum, maximum, mean, median, and IQR calculations. A column with any non-numeric
present value is treated as text and copied verbatim — `NaN` and `inf` count as
non-numeric, so a column containing them is not scaled.

</details>

<details>
<summary>What do range, ddof, and the robust toggles do?</summary>

**Range minimum / maximum** set the target interval for min-max (default `0`–`1`;
the minimum must be less than the maximum). **Ddof** picks the z-score standard
deviation: `0` divides by *n* (population), `1` divides by *n − 1* (sample). For
robust scaling, **Subtract the median** and **Divide by the IQR** each toggle one
step — turn either off to center only or scale only.

</details>

<details>
<summary>What happens when a column is constant or has zero spread?</summary>

If a column's values are all equal there is nothing to spread out: min-max returns
the range minimum, z-score returns `0` (its standard deviation is zero), and robust
scaling divides by `1` instead of a zero IQR (so with centering on you get `0`).

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole computation runs locally with WebAssembly; your CSV never leaves your
browser.

</details>

### Limits & edge cases

- Scaling needs numbers: a column with any non-numeric present value is treated as
  text and copied unchanged. Use a separate cleanup step for categorical data.
- Scaled values are rounded to 6 decimal places and printed minimally (`1.0` → `1`).
- Rows may have different column counts; short rows are padded with empty (blank)
  cells to the widest row before scaling.
- Turn off **First row is a header** for files with no header row; then column
  selection must use 1-based indexes and row 1 is scaled like any other.
- Range, ddof, and the robust toggles only affect their own method; e.g. changing
  the range has no effect under z-score or robust.
- Everything runs in memory, so very large files are bounded by your browser's
  available memory.

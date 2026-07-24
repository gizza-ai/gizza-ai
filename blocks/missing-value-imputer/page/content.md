## Fill missing values in a CSV

Paste a CSV with blank or `NA` cells and this tool fills them in locally in your
browser — **nothing is uploaded**. Pick the imputation strategy that fits your
data:

- **Mean** — replace blanks in a numeric column with the column average.
- **Median** — replace blanks with the middle value; robust to outliers.
- **Most frequent** — replace blanks with the column's most common value; works on
  text (categorical) columns too.
- **Constant** — write a fixed value (like `0` or `Unknown`) into every blank.
- **KNN** — estimate each blank from its *k* nearest rows using a nan-euclidean
  distance over the numeric columns, averaged uniformly or by inverse distance.

A cell counts as **missing** when it is empty after trimming, or equals one of the
extra markers you list under *missing markers* (for example `NA`, `null`, `?`).
Mean, median, and KNN only touch **numeric** columns (a column whose every present
value parses as a number); most-frequent and constant apply to any column.

### Worked example

With the default **Mean** strategy, this input:

```
name,age,city
Alice,30,NYC
Bob,,LA
Carol,40,
```

becomes:

```
name,age,city
Alice,30,NYC
Bob,35,LA
Carol,40,
```

`age` is numeric, so Bob's blank is filled with the mean of `30` and `40` — `35`.
The blank `city` for Carol is left untouched because `city` is a text column and
mean/median only impute numeric data; switch to **Most frequent** or **Constant**
to fill it.

### FAQ

<details>
<summary>Which columns get imputed?</summary>

By default every applicable column. Type a comma-separated list under **Columns to
impute** to restrict it — use header names (like `age,income`) when the first row
is a header, or 1-based indexes (like `2,3`). Columns you don't list are copied
through unchanged.

</details>

<details>
<summary>What counts as a missing value?</summary>

A cell is missing when it's empty or whitespace-only after trimming. To also treat
sentinel strings as missing, list them under **Extra missing markers** —
`NA,null,?` for example. Matching is exact and case-sensitive, so `NA` is missing
but `na` is not unless you add it too.

</details>

<details>
<summary>How does the mean/median differ from most-frequent and constant?</summary>

**Mean** and **median** are numeric: they only fill columns whose present values
are all numbers, and they skip text columns entirely. **Most frequent** picks the
most common existing value (ties go to the first one seen) and works on any column,
including categories. **Constant** ignores the data and writes whatever you put in
**Fill value** into every blank.

</details>

<details>
<summary>How does KNN imputation work here?</summary>

For each missing numeric cell, the tool measures the distance from that row to
every other row that *has* a value in the column, using a nan-euclidean metric over
the numeric columns (it scales by the number of shared present features). It then
averages the `n_neighbors` closest rows — a plain average with **Uniform** weights,
or an inverse-distance weighted average with **Distance**. Rows with no comparable
neighbour fall back to the column mean.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole imputation runs locally with WebAssembly; your CSV never leaves your
browser.

</details>

### Limits & edge cases

- Mean, median, and KNN need numbers: a column with any non-numeric present value
  is treated as text and left unchanged by those strategies. Use most-frequent or
  constant for categorical data.
- `NaN` and `inf` are **not** valid numbers here — a column containing them is
  treated as text, not numeric.
- Rows may have different column counts; short rows are padded with empty (missing)
  cells to the widest row before imputing.
- Turn off **First row is a header** for files with no header row; then column
  selection must use 1-based indexes and row 1 is imputed like any other.
- KNN with no usable neighbour (e.g. every other row also blank in that column)
  falls back to the column mean, and to nothing if the column is entirely empty.
- Everything runs in memory, so very large files are bounded by your browser's
  available memory.

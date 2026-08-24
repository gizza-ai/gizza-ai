## About this tool

Percentile rank answers a relative-position question: "What percentage of this dataset is at or
below my value?" It is useful for test scores, response times, salaries, lab measurements, model
scores, and any quick descriptive-statistics check where the raw number matters less than where it
falls in the distribution.

Paste a reference dataset, then enter one or more target values. The report gives each target's
percentile rank, how many dataset values are below/equal/above it, the quartile, and a z-score. You
can also include a compact dataset summary with n, min, max, range, mean, median, sample standard
deviation, Q1, Q3 and IQR.

### Worked example

**Dataset:** `6, 12, 13, 17, 17, 18, 20, 23, 24, 24, 25, 26, 27, 27, 30, 32, 33`

**Value to rank:** `25`

**Method:** `weak` (count values less than or equal to the target)

There are 17 numbers, and 11 of them are less than or equal to 25, so the percentile rank is:

`11 / 17 × 100 = 64.71`

The report also shows `below: 10`, `equal: 1`, `above: 6`, quartile `Q3`, and a positive z-score
because 25 is above the dataset mean.

### Tie handling methods

Different calculators disagree when the target value is tied with values already in the dataset.
This tool exposes the convention instead of hiding it:

- **weak** — `count(values ≤ target) / n × 100`. This is the common online-calculator default.
- **strict** — `count(values < target) / n × 100`.
- **mean** — midpoint of strict and weak, so tied values split evenly.
- **rank** — average ranking of tied values, matching SciPy's `percentileofscore(kind="rank")`.

When the target is not tied with a dataset entry, these methods usually agree. Values below the
minimum rank at 0; values above the maximum rank at 100.

### Limits and edge cases

- **Dataset size:** up to 10,000 numbers.
- **Targets:** up to 100 values per run.
- **Input separators:** commas, semicolons, spaces, tabs, and newlines all work.
- **Numbers only:** NaN, infinity, blanks-only input, and non-numeric tokens are rejected with an
  explicit error.
- **Rounding:** 0 to 6 decimal places. Trailing zeros are trimmed in the output.
- **Single-value datasets:** percentile ranks still work, but sample standard deviation and z-score
  are shown as `n/a`.
- **Percentile rank vs percentile value:** this tool ranks a value inside a dataset. If you need the
  dataset value at the 90th percentile, use a percentile-value calculator instead.

## FAQ

<details>
<summary>What is percentile rank?</summary>

Percentile rank is the percentage of values in a reference dataset that fall at or below a target
value, depending on the tie-handling method you choose. A percentile rank of 64.71 means the target
is higher than or equal to about 64.71% of the dataset under the selected convention.

</details>

<details>
<summary>Why do different calculators give different percentile ranks for the same data?</summary>

Ties are the usual reason. If the target value appears in the dataset, one calculator may count the
tied values, another may exclude them, and another may split them. Use `weak` for the common
"less than or equal" formula, `strict` for "less than", `mean` to split ties, or `rank` to match
SciPy-style average ranks.

</details>

<details>
<summary>Can I rank multiple values at once?</summary>

Yes. Put several target values in the "Value(s) to rank" box, separated by commas, spaces,
semicolons, or newlines. The same sorted reference dataset and tie method are used for every value,
so you can compare scores side by side.

</details>

<details>
<summary>Is this the same as finding the 90th percentile of a dataset?</summary>

No. Percentile rank starts with a value and asks where it falls. A percentile-value calculation
starts with a percentage, such as 90%, and asks which dataset value sits there. Those are inverse
questions, and interpolation rules make them different in practice.

</details>

<details>
<summary>What does the z-score in the report mean?</summary>

The z-score says how many sample standard deviations the target is above or below the dataset mean.
It is included as a quick companion statistic, but percentile rank is usually easier to explain for
skewed or tied data.

</details>

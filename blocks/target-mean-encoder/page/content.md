## About this tool

Target Mean Encoder replaces a categorical CSV column with the mean of a numeric
target for each category — also called mean, impact, or likelihood encoding. It
turns a high-cardinality text column (city, product, merchant, ZIP) into a single
model-ready numeric feature without one-hot exploding your column count.

Pick the category column and the numeric target column, and each category becomes
the average target value for its rows. Optional **m-estimate smoothing** blends
rare categories toward the overall mean, and **leave-one-out** excludes each row's
own target so an in-place encoding leaks less signal into the model.

### Worked example

CSV:

```csv
city,churn
A,1
B,1
A,0
B,1
C,1
B,0
```

With category `city`, target `churn`, replace output, and 3 decimals, each city
becomes its churn rate — A = 1/2 = 0.500, B = 2/3 = 0.667, C = 1/1 = 1.000:

```text
city,churn
0.500,1
0.667,1
0.500,0
0.667,1
1.000,1
0.667,0
```

Switch output to **append** to keep the original column and add
`city_target_enc` instead of overwriting `city`.

## Limits & edge cases

- The target column must be numeric. Non-numeric target cells are ignored when
  computing a category's mean; a column with no numbers at all is an error.
- Smoothing uses the m-estimate blend `(sum + m·prior) / (count + m)`, where
  `prior` is the global target mean. `m = 0` gives the raw category mean.
- With leave-one-out on, a category that appears in only one row has no other
  rows to average, so that row falls back to your unknown-category setting.
- Blank category cells (and categories with no usable statistics) use the
  **Unknown / blank category** option: the global mean, `NaN`, or zero.
- Columns can be chosen by header name, or by 1-based number when the header
  checkbox is off. The category and target columns must be different.
- This tool encodes one category column against one target per run.

## FAQ

<details>
<summary>What is target (mean) encoding and when should I use it?</summary>

Target encoding replaces each category with the average of a numeric target for
that category's rows. It is handy for high-cardinality categorical features where
one-hot encoding would add too many columns, giving models a compact numeric
signal instead.

</details>

<details>
<summary>How does smoothing help with rare categories?</summary>

A category seen only once or twice has a noisy mean. Smoothing (the m-estimate)
pulls those means toward the global target mean: the higher the smoothing weight
`m`, the more a small category is shrunk toward the overall average, which reduces
overfitting to rare values.

</details>

<details>
<summary>What does leave-one-out do?</summary>

Leave-one-out excludes each row's own target value from its category statistics
before encoding that row. This reduces target leakage when you encode a training
set in place, because a row can no longer "see" its own label through the encoded
value.

</details>

<details>
<summary>What happens to blank or unseen categories?</summary>

Rows whose category cell is blank — or whose category has no usable target
statistics — take the value from the **Unknown / blank category** setting. You can
use the global mean (a safe default), `NaN` (to flag them for later handling), or
zero.

</details>

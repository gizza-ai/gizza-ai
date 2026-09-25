## What this tool does

A covariance matrix summarizes how several numeric variables move together. Each diagonal entry is a variable's variance; each off-diagonal entry is the covariance between two variables. Positive covariance means the columns tend to rise together, negative covariance means one tends to fall when the other rises, and values near zero mean little linear co-movement on the original scale.

Paste a table with one observation per row and one variable per column. A non-numeric first row is treated as variable names, or you can provide names separately. The default uses the **sample** denominator `n - 1`, matching common statistics packages and spreadsheet `COVARIANCE.S`; switch to **population** when the pasted rows are the entire population.

## Worked example

Input:

```csv
x,y
1,2
2,4
3,5
```

Sample covariance output:

```text
covariance matrix (sample denominator, n=3)
        x        y
x 1.000000 1.500000
y 1.500000 2.333333
```

The diagonal says `x` has sample variance `1` and `y` has sample variance `2.333333`; the off-diagonal covariance is `1.5` in both symmetric positions.

## Matrix variants

- **Covariance** returns the variance-covariance matrix.
- **Correlation** standardizes the covariance matrix to Pearson correlations, with a diagonal of `1`.
- **Centered data** returns each input value minus its column mean.
- **Standardized data** returns z-scores. Its covariance matrix is the correlation matrix.

Use **CSV** when you want to paste the matrix into a spreadsheet, **Markdown** for docs, and **JSON** when another program needs the numbers plus means, variances, standard deviations and total variance.

## Limits and behaviour to know about

- Maximum input: **20,000 rows** and **100 columns**.
- Every cell must be a finite number after any header row. Missing values are rejected instead of guessed.
- Correlation and standardized output are undefined for constant columns because their standard deviation is zero.
- Optional weights are frequency weights: a row with weight `2` counts as if it appeared twice.
- Weighted sample covariance uses `Σw - 1`; weighted population covariance uses `Σw`.

## FAQ

<details>
<summary>Should I choose sample or population covariance?</summary>

Choose **sample** when your rows are observations drawn from a larger process and you want the usual unbiased variance estimate. That is the default in R `cov()`, NumPy's `cov`, and spreadsheet `COVARIANCE.S`. Choose **population** only when the pasted rows are the whole population you care about, matching `COVARIANCE.P`.

</details>

<details>
<summary>What is the difference between covariance and correlation?</summary>

Covariance keeps the original units, so a height/weight covariance is in `cm·kg` and its size depends on the units you chose. Correlation divides by each variable's standard deviation, giving a unitless number from `-1` to `1`. Use covariance for modelling workflows that need the original scale; use correlation when you want comparable strength across differently scaled variables.

</details>

<details>
<summary>Can the first row contain column names?</summary>

Yes. With **First row is variable names = Auto**, a non-numeric first row is treated as labels. You can force the first row to be names with **Yes**, force it to be data with **No**, or type a comma-separated label list in **Variable names** to override the header.

</details>

<details>
<summary>How are row weights interpreted?</summary>

Weights are frequency weights. If you enter weights `2,1,1`, the first data row counts twice and the other two rows count once. This is equivalent to duplicating rows before computing the matrix, but avoids making a large pasted table larger.

</details>

<details>
<summary>Why does correlation fail for a constant column?</summary>

Correlation divides covariance by the two standard deviations. A constant column has standard deviation zero, so the division is undefined. Remove the constant column, or choose **Covariance matrix** if you only need variances and covariances.

</details>

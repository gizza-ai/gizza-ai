## About this tool

Generate reproducible multivariate-normal test data from the matrix you already know: a covariance matrix, a correlation matrix plus standard deviations, or a compact structure such as `iid(4)`, `cs(4, 0.3)` or `ar1(5, 0.7)`. The tool validates that the matrix is square and symmetric, checks whether it can be factorised, then turns independent seeded normal draws into correlated rows.

Use it for Monte Carlo fixtures, examples for statistics lessons, synthetic data with known dependence, or quick sanity checks before moving the same inputs into R, Python or a simulation pipeline. The default CSV output is easy to paste into a spreadsheet; JSON includes the achieved sample mean, covariance and correlation; stats output gives a compact target-versus-sample report.

Example: bivariate standard normals with correlation 0.8 and a fixed seed:

```bash
gizza tool correlated-sample-generator "1, 0.8; 0.8, 1" samples=5 seed=42 decimals=3
```

This returns two columns, `X1` and `X2`, whose draws are reproducible for the same seed. Increase `samples` for a closer realised correlation, or set `empirical=true` when you need the sample covariance to match the target exactly.

## Limits and edge cases

- Matrices can have up to 50 variables.
- Output is capped at 100000 rows and 200000 emitted numbers, so a 4-variable request tops out at 50000 rows.
- `cholesky` is fastest but requires a positive-definite matrix. Use `eigen` for positive-semidefinite cases such as perfect correlation.
- `empirical=true` needs more rows than variables because it standardises the generated sample before recolouring it.
- Randomness is deterministic and local: no network calls, and the same seed and inputs produce the same rows across the CLI and browser page.

## FAQ

<details>
<summary>Should I enter a covariance matrix or a correlation matrix?</summary>

Use the default covariance mode when the diagonal already contains variances and the off-diagonal entries are covariances. Choose correlation mode when the matrix has 1 on the diagonal and correlations between -1 and 1 off the diagonal; then enter `sd` values if each variable should have a standard deviation other than 1.

</details>

<details>
<summary>Why does my valid-looking matrix fail with Cholesky?</summary>

Cholesky needs the matrix to be strictly positive definite. A matrix with perfect correlation, a repeated variable or small rounding errors can be only semidefinite. Try `method=eigen`; it uses a symmetric square root and accepts positive-semidefinite matrices within the tolerance.

</details>

<details>
<summary>What does empirical mode change?</summary>

Normal random samples match the requested mean and covariance only in expectation, so a small sample will show sampling variation. `empirical=true` rescales the generated rows so their sample mean and sample covariance equal the requested targets exactly, which is useful for deterministic fixtures but less representative of random variation.

</details>

<details>
<summary>How do I make the same sample again later?</summary>

Keep the same matrix, means, method, seed and output settings. The generator uses a deterministic local PRNG rather than browser entropy, so the same inputs reproduce the same rows in the CLI, web page and tests.

</details>

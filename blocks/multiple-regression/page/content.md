## About this tool

This multiple linear regression calculator fits an **ordinary least-squares (OLS)** model
`y = b₀ + b₁·x₁ + … + bₖ·xₖ` to a data matrix you paste in — one observation per line, columns
separated by commas, tabs, semicolons or spaces. By default the **last column** is the response
(dependent variable) *y* and every other column is a predictor; point `response` at `first` or a
1-based column number to pick a different Y. Give the columns real names with **labels** so the
coefficient table and equation read in your terms (`sqft`, `rooms`, `price`) instead of `v1`, `v2`, `v3`.

For each coefficient — including the intercept — you get the **estimate**, its **standard error**,
the **t-statistic**, the two-tailed **p-value** (H₀: the coefficient is zero) and a **confidence
interval** at your chosen level. At the model level it reports **R²** and **adjusted R²** (which
penalises extra predictors), the **residual standard error** with its degrees of freedom, and the
overall **F-test** with its p-value — the joint test that at least one predictor matters. Switch the
output to **JSON** to additionally get the per-observation **fitted values** and **residuals** for
diagnostics. Everything runs in your browser; your data never leaves the page.

### Worked example

Input (`data`, with `labels = sqft,rooms,price`, so `price` is the response):

```
1,1,6
2,1,8
3,2,11
4,2,14
5,3,18
6,3,20
```

Output:

```
price = 2 + 2.333333·sqft + 1.333333·rooms

Coefficients:
  term               estimate    std error          t          p   95% CI
  (Intercept)               2     0.559384   3.575424   0.037461   [0.219859, 3.780141]
  sqft               2.333333       0.3849   6.062178   0.009007   [1.108218, 3.558449]
  rooms              1.333333     0.930949   1.432222   0.247544   [-1.62904, 4.295707]
```

*(exact numbers are shown in full on the page)* — with **R² = 0.995638**, **adjusted R² = 0.99273**
and an **F-statistic of 342.375 on 2 and 3 DF (p = 0.000288)**, both predictors are strongly
significant and the model explains almost all of the variation in `price`.

### Controls

- **Response column** — `last` (default, rightmost), `first`, or a 1-based column index; every other
  column becomes a predictor.
- **Column names** — optional comma-separated labels in data order; default `v1, v2, …`.
- **Fit an intercept** — on by default. Uncheck to force a zero Y-intercept (regression through the
  origin); this also switches R² to the uncentered definition, as statistical software does.
- **Confidence level** — for the coefficient intervals (default `0.95`); the two-tailed significance
  level is α = 1 − level.
- **Output** — a formatted summary, or JSON that adds the fitted values and residuals.

### Limits and edge cases

- You need **more observations than parameters** (rows > predictors + intercept), otherwise the
  residual degrees of freedom would be zero or negative and the fit is rejected.
- Predictors must not be **perfectly collinear** (one an exact linear combination of others) — the
  normal-equations matrix is then singular and the model is not identifiable, so the tool errors
  instead of returning unstable numbers.
- Every value must be a **finite number** and every row must have the **same number of columns**;
  blank lines are ignored.
- All predictors are treated as **numeric** — encode categorical variables as dummy (0/1) columns
  and apply any log/sqrt transforms yourself before pasting.
- p-values and confidence intervals assume the usual OLS conditions (independent, homoscedastic,
  roughly normal residuals); check the residuals (JSON output) before trusting them.

## FAQ

<details>
<summary>How do I choose which column is the dependent variable (Y)?</summary>

By default the **last (rightmost) column** is the response and everything to its left is a
predictor — the convention most calculators use. Set the response field to `first` for the first
column, or to a **1-based column number** (`2` = the second column) to use any other. Whichever
column you pick becomes *y*; all remaining columns are entered as predictors together.

</details>

<details>
<summary>What is the difference between R² and adjusted R²?</summary>

R² is the share of the variance in *y* explained by the model, from 0 to 1. It can only go **up**
when you add predictors, even useless ones, so it rewards a bigger model unfairly. Adjusted R²
corrects for that by penalising each extra predictor relative to the sample size, so it can fall
when a predictor adds nothing. Use adjusted R² when comparing models with different numbers of
predictors.

</details>

<details>
<summary>What does the p-value on each coefficient mean?</summary>

It is the two-tailed probability, under the null hypothesis that the true coefficient is **zero**,
of seeing a t-statistic at least as extreme as the one observed. A small p-value (say below your α
= 1 − confidence level) is evidence the predictor has a non-zero effect *holding the other
predictors fixed*. The overall **F-test** is the joint version: it asks whether the predictors
together explain more than chance would.

</details>

<details>
<summary>Why does forcing a zero intercept change my R²?</summary>

With an intercept, R² is computed against the mean of *y* (centered total sum of squares). When you
force the line through the origin there is no intercept to absorb the mean, so software — this tool
included — switches to the **uncentered** total sum of squares. The two R² values are not
comparable, and a no-intercept R² is often much higher for reasons that have nothing to do with fit
quality, so interpret it with care.

</details>

<details>
<summary>Why did I get a "perfectly collinear" or "not enough observations" error?</summary>

OLS needs the predictor matrix to be full rank. **Perfect collinearity** — one predictor being an
exact linear combination of the others (e.g. a column that is another times a constant, or dummy
columns that sum to 1) — makes the system unsolvable, so the tool stops instead of printing
meaningless coefficients. You also need **more rows than parameters**; with as many parameters as
observations the model would fit perfectly with zero residual degrees of freedom and no way to
estimate uncertainty. Add rows or drop a redundant predictor.

</details>

## About this tool

Least-squares regression finds the curve that minimises the sum of squared residuals between your observed `y` values and the model's fitted values. Paste two columns of data (`x,y`), choose a polynomial degree, and the calculator returns the fitted equation plus the diagnostics you need to judge the model.

The default is ordinary straight-line regression:

```text
x,y
1,2
2,4
3,5
4,4
5,5
```

With degree `1`, the output begins:

```text
y = 0.6000·x + 2.2000
```

and includes R², adjusted R², Pearson `r`, RMSE, residual standard error, coefficient estimates with standard errors, and residual min/median/max. Add values in **Predict at x values** (for example `6, 7`) to evaluate the fitted model at new points.

Set degree `2` for a quadratic, degree `3` for a cubic, or any degree up to `10` when you have enough distinct data points. Use **Separate y values** when your x and y series are copied from different spreadsheet columns. Use **Output** to switch from the readable text report to CSV tables or structured JSON.

### Limits and edge cases

- This is one-variable regression: one numeric `x` column and one numeric `y` column. Multiple regression belongs in a different tool.
- Degree must be 1–10 and the data must leave residual degrees of freedom. A degree-2 fit with an intercept estimates 3 coefficients and therefore needs at least 4 points.
- A polynomial needs enough distinct x values. Repeated x values are allowed, but a degree-3 fit cannot be identified from only two unique x coordinates.
- Very large x magnitudes and high powers can still be ill-conditioned. The solver column-scales and uses QR decomposition, but you should centre or rescale x (for example years since 2000 instead of full dates like 20260101) when fitting higher-degree polynomials.
- R² is descriptive, not proof of causation or a forecast guarantee. Extrapolated predictions outside the observed x range can be wildly unstable, especially for high-degree polynomials.
- Missing values, formulas, dates, and category labels are not imputed. Clean the data to finite numeric pairs first.

## FAQ

<details>
<summary>What is least-squares regression?</summary>

It is the standard method for fitting a line or curve by minimising the total squared vertical distance between the data points and the model. Squaring makes large misses count more than small misses and gives a deterministic closed-form fit for linear-in-the-coefficients models such as polynomials.

</details>

<details>
<summary>When should I use polynomial degree greater than 1?</summary>

Use degree 2 or 3 only when the scatterplot clearly bends and you have enough points to support the extra coefficients. Higher degrees can fit the sample very closely while behaving badly between or beyond points. If you only need a straight trendline equation and R², leave the degree at 1.

</details>

<details>
<summary>Why did the tool say the fit is rank-deficient?</summary>

The chosen degree is not identifiable from the x values you supplied. Common causes are too few distinct x values, duplicate x coordinates with a high-degree polynomial, or x values so large that powers of x become numerically unstable. Lower the degree, add more spread-out points, or centre/scale x before fitting.

</details>

<details>
<summary>What is the difference between RMSE and residual standard error?</summary>

RMSE is `sqrt(RSS / n)`, so it averages residual size over all points. Residual standard error is `sqrt(RSS / df)` where `df` subtracts the number of fitted coefficients; it is the usual regression estimate of noise scale and grows when the model spends more degrees of freedom on coefficients.

</details>

<details>
<summary>Can I force the fitted line through zero?</summary>

Yes. Turn off **Fit intercept** to fit through the origin. Do this only when the domain demands that `x = 0` implies `y = 0`; otherwise it can bias the slope and make R² harder to compare with ordinary intercept models.

</details>

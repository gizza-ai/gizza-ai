## About this tool

Spline Smoother fits a natural cubic smoothing spline to noisy numeric data. Paste one y value per row, x,y rows with an optional header, a one-line list of y values, or JSON arrays/objects. The tool sorts by x, merges exact duplicate x values by weighted mean, and reports fitted values, residuals, leverage, effective degrees of freedom, selected lambda, RMSE, and optional predictions.

Use `mode=auto` when you want the tool to pick a penalty by generalized cross-validation or leave-one-out CV. Use `mode=smoothing` for a scale-free p slider: `0` is the weighted least-squares straight line and `1` interpolates the distinct data points. Advanced workflows can provide a raw non-negative `lambda` or a target effective `df`.

### Worked example

Input:

```text
x,y
1,2.1
2,3.9
3,6.2
4,7.8
5,10.3
6,11.7
```

With `mode=auto`, `criterion=gcv`, and `output=json`, the result includes a JSON report with the chosen smoothing penalty, fitted values for every observation, residuals, and RMSE. Switch `output=svg` for a quick raw-versus-fit chart, or set `predict_at=1.5,3.5,5.5` to evaluate the fitted curve at custom x values.

### Limits and edge cases

- Requires at least 4 distinct numeric x values.
- Accepts up to 10,000 input points, 5,000 prediction x values, and 5,000 resampled curve points.
- Input text is capped at 2 MB.
- Date/time x axes are not parsed directly; convert dates to day indexes or epoch seconds before fitting.
- Exact duplicate x values are merged before fitting using a weighted mean.
- `smoothing=1` interpolates the distinct points; `smoothing=0` returns the straight-line least-squares limit.

## FAQ

<details>
<summary>What input formats can I paste?</summary>

You can paste one y value per row, two-column x,y rows separated by commas, spaces, tabs, or semicolons, a one-line list such as `10, 12, 11, 15`, a JSON array of numbers, a JSON array of `[x, y]` pairs, or JSON objects with `x` and `y` fields. A non-numeric first row is treated as a header.

</details>

<details>
<summary>How should I choose between auto, smoothing, lambda, and df?</summary>

Start with `auto` and `criterion=gcv` for an objective default. Use `smoothing` when you want a slider-style control where lower values are smoother and `1` interpolates. Use `lambda` if you need to reproduce a known penalized-spline setting, or `df` when you want the fitted curve to have a particular effective degrees of freedom.

</details>

<details>
<summary>What does the coefficients option return?</summary>

When `coefficients=true`, JSON and CSV output include one row per interval with `x_start`, `x_end`, and cubic coefficients `a`, `b`, `c`, `d`. On that interval the fitted curve is `a + b·(x-x_start) + c·(x-x_start)^2 + d·(x-x_start)^3`.

</details>

<details>
<summary>Does the tool handle irregular spacing and duplicate x values?</summary>

Yes. X values can be irregular and unsorted. The tool sorts them before fitting. Exact duplicate x values are merged into one distinct knot using the weighted mean of their y values, and the report states how many duplicates were merged.

</details>

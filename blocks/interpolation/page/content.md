## About this tool

Use this interpolation calculator when you have measured points and need values between them: lab readings at uneven times, calibration curves, lookup tables, sampled cumulative totals, or a small engineering dataset copied from a spreadsheet. Paste the known points as one `x,y` pair per line, separated by commas, spaces, tabs, or semicolons. A single y-only column is accepted too — x becomes `1, 2, 3…` — and JSON forms such as `[[x,y], …]` or `[{"x": 1, "y": 10}, …]` work for automation.

Choose **linear** for straight segments, **cubic spline** for a smooth curve, **monotone PCHIP** when the result must not overshoot between points, **polynomial** for the single curve through every point, or **nearest neighbour** for step/lookup-table data. The cubic option supports natural, not-a-knot, and clamped-slope boundaries. Enter explicit x values in **Evaluate at x**, or set **Resample count** to get evenly spaced samples across the full data range. The default with both blank is to evaluate every interval midpoint.

The output can be plain `x,y` values, CSV with source/extrapolation columns, JSON for scripts, or a self-contained SVG chart. Turn on **Include fitted equations / coefficients** to see the segment equations for linear, cubic, monotone, and nearest methods, or the polynomial coefficients for polynomial mode. **Derivative order** reports `dy/dx` or `d²y/dx²` instead of y, and **Outside range** decides whether extrapolation is rejected, clamped to an endpoint, or extended from the end segment.

**Worked example.** Paste this dataset, choose `cubic`, set **Cubic boundary** to `not-a-knot`, and evaluate at `1.5, 2.5`:

```text
0,0
1,1
2,8
3,27
4,64
```

The not-a-knot spline reproduces the underlying `y = x³` curve for these points, so the values are `1.5,3.375` and `2.5,15.625`. Turn on coefficients to inspect the pieces, or switch to SVG to see the fitted curve against the anchor points.

**Limits and edge cases.** Data input is capped at 2 MB and 10,000 points. Polynomial interpolation is capped at 30 points because high-degree fits oscillate rapidly; the tool warns above 10 points. Evaluation lists and resampling grids are capped at 5,000 x values. Duplicate x values are rejected; unsorted rows are sorted and reported as a warning. Interpolation outside the known range is refused by default because extrapolation can be misleading.

## FAQ

<details>
<summary>Which method should I pick?</summary>

Use **linear** when straight-line changes are acceptable and you want the most transparent answer. Use **cubic spline** for a smooth curve with smooth derivatives. Use **monotone PCHIP** for cumulative totals, concentrations, percentages, or any data where overshoot would be physically impossible. Use **polynomial** only for a small number of points that really should lie on one global curve. Use **nearest neighbour** for lookup tables, labels, or step functions.

</details>

<details>
<summary>What is the difference between natural, not-a-knot, and clamped cubic splines?</summary>

A **natural** spline sets the curvature to zero at both ends, which keeps the ends from bending sharply and is a common textbook default. **Not-a-knot** removes the first and last internal knots, so samples from a true cubic are reproduced exactly; many numerical packages use this convention. **Clamped** lets you supply the starting and ending slopes, which is useful when the endpoint rates are known from physics or instrumentation.

</details>

<details>
<summary>Why can polynomial interpolation look wild between points?</summary>

A polynomial through every point is a single high-degree curve. As the point count grows, especially near the ends of the range or with evenly spaced x values, that curve can swing far away from the data even though it passes through every anchor. This is Runge's phenomenon. For most real measured data, a cubic spline or monotone interpolation is safer.

</details>

<details>
<summary>Can I extrapolate beyond my first or last point?</summary>

Yes, but it is off by default. Set **Outside range** to **Clamp** to return the nearest endpoint value, or **Extend** to continue the first or last fitted segment. The output marks extrapolated rows and adds a warning. Treat those numbers as a trend estimate, not as interpolation backed by data.

</details>

<details>
<summary>Are the decimals used in the calculation?</summary>

No. The interpolation runs with 64-bit floating-point values. **Decimal places** only controls how numbers are printed in values, CSV, JSON, SVG labels, and equations. Increase it for golden-file comparisons or lower it for reports meant for people.

</details>

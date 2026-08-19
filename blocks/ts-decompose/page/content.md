## About this tool

Time series decomposition separates an evenly spaced series into the slow-moving
**trend**, the repeating **seasonal** pattern, and the leftover **residual**. It
is the quick way to answer questions like "is demand really growing, or is this
just the December peak?" and "which points are unusual after accounting for the
weekly cycle?"

Paste values one per line, `label,value` rows, or a single comma/space-separated
line. Set `period` when you know the cycle length — 12 for monthly data with a
yearly pattern, 4 for quarterly, 7 for daily data with a weekly pattern, 24 for
hourly data with a daily pattern — or leave it at 0 to detect the strongest
autocorrelation peak.

The default **STL** method uses a local smoother and can run a robust pass that
keeps outliers from bending the trend. The **classical** method uses a centred
moving average and one fixed seasonal index for each position in the cycle. Use
an additive model when the seasonal swing is roughly constant, and a
multiplicative model when the swing grows with the level.

### Worked example

Input:

```text
Jan,104
Feb,102
Mar,99.5
Apr,98.5
May,97
Jun,101
Jul,104.5
Aug,107
Sep,110.5
Oct,107.5
Nov,103
Dec,102.5
Jan,110
Feb,108
Mar,105.5
Apr,104.5
May,103
Jun,107
Jul,110.5
Aug,113
Sep,116.5
Oct,113.5
Nov,109
Dec,108.5
```

With `period=12`, the SVG output shows four stacked panels: observed values,
trend, seasonal component, and residual. Switch output to `table`, `csv`, or
`json` when you need the exact component values, seasonal indices, and the
strength-of-trend / strength-of-seasonality diagnostics.

### Limits and edge cases

- Input is capped at 10,000 observations and needs at least two full seasonal
  cycles. For `period=12`, paste at least 24 values.
- Multiplicative decomposition requires every value to be greater than zero.
  Use additive mode for data that can be zero or negative.
- `period=0` uses autocorrelation and can fail on very short or trend-only
  series. Set the period explicitly when you know the sampling cadence.
- The chart is an SVG string, so it can be copied, downloaded, or embedded in a
  report. No network call or plotting service is used.

## FAQ

<details>
<summary>What period should I choose?</summary>

Use the number of observations in one repeating cycle. Monthly data with yearly
seasonality uses `12`, quarterly uses `4`, daily data with a weekly pattern uses
`7`, and hourly data with a daily pattern uses `24`. Leave `period=0` only when
you want the tool to infer the strongest cycle from autocorrelation.

</details>

<details>
<summary>When should I use STL instead of classical decomposition?</summary>

Use STL when the seasonal shape can drift over time or when you want `robust`
mode to isolate outliers. Use classical decomposition when you want the textbook
moving-average trend and one fixed seasonal index for each point in the cycle.

</details>

<details>
<summary>What is the difference between additive and multiplicative models?</summary>

Additive mode assumes `observed = trend + seasonal + residual`, so the seasonal
swing has about the same size across the whole series. Multiplicative mode
assumes `observed = trend × seasonal × residual`, so seasonal swings scale with
the series level; it requires strictly positive data.

</details>

<details>
<summary>Why did automatic period detection fail?</summary>

The detector looks for a strong autocorrelation peak. Trend-only data, very noisy
series, or fewer than two cycles may not have a trustworthy peak. Set `period`
explicitly and rerun; if the data is monthly, start with `12`.

</details>

<details>
<summary>Can I export the component values?</summary>

Yes. Use `output=csv` for one row per observation, `output=json` for diagnostics
plus component arrays, or `output=table` for a readable text table. The SVG is
best for visual reporting; the other formats are best for downstream analysis.

</details>

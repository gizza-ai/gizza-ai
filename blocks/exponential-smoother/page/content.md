## About this tool

Exponential Smoother applies an exponentially weighted moving average (EWMA) to a numeric series. Every past observation still counts, but its weight decays geometrically, so the result tracks the level of noisy data without the lag and the hard cut-off of a fixed-window average.

The decay is one number, `alpha`, and you can supply it however your reference does:

- `mode=alpha` — the smoothing factor itself, `0 < alpha <= 1`
- `mode=span` — the familiar N-period EMA setting, `alpha = 2 / (span + 1)`
- `mode=halflife` — periods until an observation's weight halves, `alpha = 1 - exp(-ln2 / halflife)`
- `mode=com` — center of mass, `alpha = 1 / (1 + com)`
- `mode=auto` — fit alpha by minimising the sum of squared one-step-ahead forecast errors

Whichever you pick, the report echoes all four equivalents so you can move between conventions. Set `adjust=false` for the plain simple-exponential-smoothing recursion `y_t = (1 - alpha)·y_(t-1) + alpha·x_t` used by finance EMAs, or leave `adjust=true` for the bias-corrected form that divides by the decaying weight sum. Everything runs locally in your browser — the numbers never leave the device.

### Worked example

Input:

```text
12
14
13
17
16
20
19
24
```

With `mode=alpha`, `alpha=0.3` and `adjust=true`, the smoothed series comes back as `12`, `13.1765`, `13.0959`, `14.6372`, `15.1286`, `16.7849`, `17.5091`, `19.5755`. The first value is just the first observation. The second is `(14 + 0.7·12) / (1 + 0.7) = 13.1765` and the third `(13 + 0.7·14 + 0.49·12) / 2.19 = 13.0959`, so the dip at period 3 barely moves the level. The final `level` is `19.5755`. The JSON report also carries the equivalent `span` of `5.66667`, a `halflife` of `1.94336` and a `com` of `2.33333`, plus one-step-ahead errors (`rmse` `3.62333`, `mae` `3.00297`, `mape` `15.741`) so you can compare two settings on the same data.

Set `forecast=6` to project six periods ahead. Simple exponential smoothing has a flat forecast function, so all six sit at that final level.

### Limits and edge cases

- Accepts up to 20,000 data points and 2 MB of input text; `forecast` is capped at 1,000 periods.
- `alpha` must be greater than 0 and at most 1. `alpha=1` reproduces the input exactly; there is no valid `alpha=0`.
- `span` must be at least 1, `halflife` greater than 0, and `com` 0 or greater.
- A leading text header row (for example `value`) is skipped; `na`, `n/a`, `nan`, `null`, `none`, `-`, `.` and `?` mark a missing observation. A gap is echoed as `null` in `values`, but `smoothed` carries the previous level forward.
- Warm-up: `min_periods=0` and `min_periods=1` both emit a value from the first observation. Higher values return `null` until that many observations have been seen.
- Error metrics are one-step-ahead: the forecast for a period is the smoothed level after the previous period. `mape` skips periods whose actual value is 0.
- The series is treated as evenly spaced. Irregular timestamps and date-based half-lives are not supported — resample to a fixed interval first.
- All reported numbers are rounded to 6 significant digits.

## FAQ

<details>
<summary>What is the difference between alpha and span?</summary>

They are two ways of writing the same decay. `span` is the "N-period EMA" convention used by charting and finance tools, and it converts as `alpha = 2 / (span + 1)` — so a 12-period EMA is `alpha = 0.153846`. Use `span` when you are reproducing an N-day EMA, and `alpha` when you want a decay rate that no integer span can express, such as `0.25`. The JSON report prints both, plus the equivalent half-life and center of mass.

</details>

<details>
<summary>Should I turn "adjust" on or off?</summary>

Leave it on when you are summarising data and want the early points to be honest weighted averages of everything seen so far. Turn it off when you are reproducing a textbook simple exponential smoothing table or a finance EMA, both of which use the plain recursion `y_t = (1 - alpha)·y_(t-1) + alpha·x_t` seeded at the first observation. The two agree more and more as the series gets longer; they differ most in the first few periods, where `adjust=true` divides by the partial weight sum instead of leaning on the seed value.

</details>

<details>
<summary>How does auto-fit choose alpha?</summary>

It scans alpha across `(0, 1]` and then refines the best region with a golden-section search, minimising the sum of squared one-step-ahead forecast errors — the standard criterion for picking a simple exponential smoothing constant. A trending series usually fits a high alpha because following the data closely beats averaging it; noise around a stable mean fits a low alpha. Auto-fit needs at least two numeric values, and it honours whatever `adjust`, `ignore_na` and `min_periods` you have set.

</details>

<details>
<summary>How are missing values handled?</summary>

A gap keeps its slot so your rows stay aligned: it comes back as `null` in the echoed `values`, while `smoothed` carries the last level forward — the smoothed level is defined at every period, and a missing observation simply does not update it. Gaps are also skipped when scoring, so they never contribute a forecast error. What changes is the weighting of the points around the gap. With `ignore_na=false` (the default) a gap still consumes one step of decay, so an observation after a long gap counts for less. With `ignore_na=true` the gap is skipped entirely and weights are assigned by position among the actual observations, as if it were never there.

</details>

<details>
<summary>Can it forecast a trend or a seasonal pattern?</summary>

No. This is single (simple) exponential smoothing, which models level only, so its forecast is flat at the last smoothed value. That is the correct forecast for a series without trend or seasonality, and it is still useful as a baseline for one that has them. Double exponential smoothing (Holt's linear trend) and triple exponential smoothing (Holt-Winters, with seasonality) are separate models and are not implemented here.

</details>

<details>
<summary>What do the error metrics mean?</summary>

They score the one-step-ahead forecasts: for each period, the forecast is the smoothed level after the previous period, and the error is the actual value minus that forecast. `sse` is the sum of squared errors, `mse` its mean, `rmse` the square root of `mse` (in the units of your data), `mae` the mean absolute error, and `mape` the mean absolute percentage error. Lower is better, and comparing `rmse` across two alpha values is the usual way to decide which smoothing is doing more good than harm on your series.

</details>

# time-series-forecaster competitor analysis (2026-08-18)

Backlog tool: `time-series-forecaster` — Forecast future points of a univariate time series with exponential smoothing/ARIMA and confidence bands.

## Sources scanned

Searches used:
- `online time series forecasting exponential smoothing Holt Winters forecast calculator tool`
- `time series forecasting tool Holt Winters exponential smoothing prediction intervals online`

Representative competitors / references from the result set:

| Source | Table-stakes observed | In model? | Decision for this tool |
| --- | --- | --- | --- |
| MetricGate Holt-Winters calculator/docs | Additive and multiplicative Holt-Winters, alpha/beta/gamma controls, season length, worked explanation. | Yes | Implemented Holt-Winters additive and multiplicative models; exposed `season_length`, `alpha`, `beta`, and `gamma`; documented two-cycle requirement. |
| PlanetCalc triple exponential smoothing calculator | Online paste-and-calculate smoothing workflow with seasonal period, forecast horizon, and smoothing coefficients. | Yes | Added browser textarea input, horizon control, season length, and optional fixed smoothing weights. |
| Real Statistics Holt-Winters examples | Worked examples with explicit smoothing parameters and seasonal cycles. | Yes | Added examples/preset chips for trend and seasonal series; pinned weights are respected when supplied. |
| Forecasting: Principles and Practice Holt-Winters material | Additive vs multiplicative seasonality distinction and forecast visuals/bands as common expectations. | Partly | Additive/multiplicative models are implemented; charting is out-of-model for the current generic text-output page, so the tool returns tables and intervals instead. |
| Spreadsheet/statistics packages such as SigmaXL/Excel workflows | Rich charts, worksheets, residual diagnostics and model families beyond ETS. | Partly / no | Kept deterministic in-browser ETS forecasts, accuracy metrics, fitted residual table, CSV/JSON exports. ARIMA, regressors, holiday effects and interactive charts are listed as limits rather than built. |

## Table-stakes checklist

| Capability / UX pattern | Fit | Implemented as |
| --- | --- | --- |
| Paste values with optional labels | In-model | `data` string parser accepts one value per line, labelled rows, CSV/semicolon/tab/space splitting, and a one-row series. |
| Forecast horizon | In-model | `horizon` integer, 1–240; page slider covers 1–60. |
| Season length | In-model | `season_length` integer, 0–366; seasonal models require `>=2` and two full cycles. |
| Model choice | In-model | `auto`, `simple`, `holt`, `damped`, `holt-winters-additive`, `holt-winters-multiplicative`. |
| Auto model selection | In-model | Deterministic grid fitting plus AICc ranking for applicable candidates. |
| Smoothing weights | In-model | `alpha`, `beta`, `gamma`, `phi` sliders; `0` means fit automatically and positive values pin the parameter. |
| Confidence / prediction bands | In-model | `confidence` enum for 80/90/95/99 percent residual-based bands. |
| Accuracy metrics | In-model | MAE, RMSE, MAPE where defined, MASE where defined, residual sigma and AICc in output. |
| Fitted values / residuals | In-model | `show_fitted` checkbox emits per-period fitted table. |
| Export formats | In-model | `format` enum: text, CSV sections, structured JSON. |
| Preset worked examples | In-model | Four page `[[example]]` chips: trend, seasonal, damped, JSON. |
| Charts | Out-of-model for this page generator pass | Documented as a limit; text/CSV/JSON tables remain exact and testable. |
| ARIMA / SARIMA / regressors / holiday effects | Out-of-model for pure deterministic browser block | Documented as limits; no ML, Python, or server-side stats package. |
| Missing-timestamp calendar inference | Out-of-model | Input is an ordered univariate sequence; labels are displayed but not used as dates. |

## Defaults chosen

- `model=auto` to match calculators that help users choose a baseline.
- `horizon=6` as a modest planning horizon.
- `season_length=0` to keep non-seasonal data easy; seasonal models explain the required cycle length.
- `alpha=beta=gamma=phi=0` means automatic fitting, while pinned positive weights support reproducible examples.
- `confidence=95`, `format=text`, `decimals=3`, `show_fitted=false`, `header=auto`.

## Verification focus derived from scan

- Exact CLI/page output for a Holt trend example.
- Deep-link coverage for non-default model, horizon, confidence and format.
- Matrix coverage for every model enum, every confidence enum, CSV/JSON/text formats, non-default checkbox, max horizon boundary, max point-count boundary, and validation errors for seasonal and multiplicative constraints.

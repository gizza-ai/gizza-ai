## About this tool

Forecast a single numeric series without leaving the browser. Paste one value per row, a labelled
CSV-style row such as `Jan,120`, or a single comma-separated series, then choose an exponential
smoothing model: simple level smoothing, Holt's linear trend, a damped trend, or Holt-Winters
seasonality. If you leave **Model** on auto, the tool fits every applicable candidate and picks the
lowest-AICc result.

The output reports the selected model, fitted smoothing weights, in-sample accuracy metrics, and a
future forecast table with lower and upper prediction bands. Optional fitted values make it easier
to see where the model tracked the history well and where residuals were large. Everything is
computed locally with deterministic WebAssembly, so the page, CLI, and chat tool use the same core
logic.

### Worked example

Paste this monthly sales series:

```text
month,sales
Jan,120
Feb,132
Mar,141
Apr,158
May,166
Jun,181
Jul,190
Aug,203
```

Set **Model** to `holt`, **Periods to forecast** to `3`, **Prediction interval** to `95%`, and
**Decimal places** to `2`. The forecast continues the fitted trend and returns a table like:

```text
period forecast lower upper
9      ...      ...   ...
10     ...      ...   ...
11     ...      ...   ...
```

Use **Holt-Winters additive** with a season length such as `12` for monthly data with a yearly cycle,
or `4` for quarterly data. Multiplicative seasonality is useful when seasonal swings grow with the
level, but it requires strictly positive observations.

### Limits and edge cases

- Input needs at least **3 observations** and accepts at most **10,000 observations**.
- Forecast horizon accepts **1–240 periods**; the page slider focuses on the common 1–60 range.
- Seasonal models need `season_length >= 2` and at least **two full cycles** of history.
- Smoothing weights (`alpha`, `beta`, `gamma`, `phi`) left at `0` are fitted automatically; set a
  positive value to pin that weight.
- Multiplicative Holt-Winters rejects zero or negative observations because seasonal ratios would be
  undefined.
- Prediction intervals are approximate ETS residual bands, not a guarantee. Damped and
  multiplicative seasonal intervals are especially approximate at long horizons.
- `MAPE` is omitted when actual values are zero; `MASE` needs enough naive seasonal differences to
  establish a scale.
- This is a univariate forecaster. It does not model promotions, regressors, holidays, multiple
  series, missing timestamps, or machine-learning features.

## FAQ

<details>
<summary>Which model should I choose?</summary>

Use **auto** when you want a quick baseline: it tries the applicable exponential-smoothing models
and chooses by AICc. Use **simple** for a mostly flat series, **Holt** for a clear trend,
**damped** when the trend should flatten over time, and **Holt-Winters** when the same seasonal
pattern repeats every `season_length` observations.

</details>

<details>
<summary>What does season length mean?</summary>

Season length is the number of observations in one repeat cycle. Monthly data with yearly seasonality
uses `12`, quarterly data uses `4`, weekly data with daily observations uses `7`, and non-seasonal
data uses `0`. Seasonal models need at least two complete cycles so the starting seasonal indices can
be estimated.

</details>

<details>
<summary>Do I need to set alpha, beta, gamma, or phi?</summary>

Usually no. Leave them at `0` and the tool fits deterministic values by minimising one-step-ahead
squared errors. Set a positive value only when you need to reproduce a known model or compare a
specific smoothing weight against the automatic fit.

</details>

<details>
<summary>Can I paste labelled rows or currency values?</summary>

Yes. Rows like `Jan, $120`, percent-marked values, underscores, and accounting negatives such as
`(12.5)` are normalised before fitting. The last field in each row is treated as the numeric value;
earlier fields become labels for the optional fitted table.

</details>

<details>
<summary>Are the prediction intervals statistical confidence guarantees?</summary>

No. They are practical residual-based forecast bands for the selected ETS model. They are useful for
rough planning and anomaly checks, but they do not replace a full statistical modelling workflow,
especially for intermittent demand, structural breaks, or long seasonal horizons.

</details>

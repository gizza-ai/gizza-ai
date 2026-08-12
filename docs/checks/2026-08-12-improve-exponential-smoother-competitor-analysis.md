# exponential-smoother — competitor analysis (2026-08-12)

Scan run **before** implementing, per `.claude/skills/create-tool-loop/SKILL.md` step 4.
All notes are paraphrased observations of publicly documented behaviour — no competitor copy,
branding or trademarks are reproduced, and out-of-model items are listed, not built.

## Search

Two WebSearches: *"exponential smoothing EWMA online calculator alpha span smoothing factor tool"*
and *"online exponential smoothing calculator single exponential smoothing forecast alpha data
series"*. The space splits cleanly in two: library/reference implementations that define the
parameter conventions, and hosted single-exponential-smoothing calculators that define the
worked-table/forecast UX. Three were skimmed, one from the first group and two from the second:

| # | Tool | Reached | Role |
|---|------|---------|------|
| 1 | pandas `DataFrame.ewm` (docs) | yes | de-facto reference for the EWMA parameter set and the adjust/ignore_na weighting definitions |
| 2 | MathCracker exponential smoothing forecast calculator | yes | hosted SES calculator; smoothing-constant + optional seed + period-label UX |
| 3 | Wessa exponential smoothing (free statistics software) | yes | hosted; fitted parameters, forecast horizon, fitted-vs-observed tables and charts, CSV export |

## Table stakes observed

| # | Capability | Seen in | Verdict | Where it landed |
|---|------------|---------|---------|-----------------|
| 1 | Smoothing factor `alpha` given directly, `0 < alpha <= 1` | 1, 2, 3 | in-model | `mode=alpha` + `alpha` param (page slider) |
| 2 | N-period span convention, `alpha = 2/(span+1)` | 1 | in-model | `mode=span` + `span` param (page slider) |
| 3 | Half-life convention, `alpha = 1 - exp(-ln2/halflife)` | 1 | in-model | `mode=halflife` + `halflife` param (page slider) |
| 4 | Center-of-mass convention, `alpha = 1/(1+com)` | 1 | in-model | `mode=com` + `com` param (page slider) |
| 5 | Bias-corrected weighting vs the plain recursion (`adjust`) | 1 | in-model | `adjust` boolean, default true; `false` gives `y_t = (1-a)y_{t-1} + a x_t` |
| 6 | Missing-value handling with a weighting switch (`ignore_na`) | 1 | in-model | gap tokens in the parser + `ignore_na` boolean |
| 7 | Warm-up suppression (`min_periods`) | 1 | in-model | `min_periods` param (0 and 1 both emit from the first observation) |
| 8 | Automatically fitted smoothing constant | 3 (fits alpha via the underlying stats routine) | in-model | `mode=auto` — grid + golden-section on one-step-ahead SSE |
| 9 | Forecast N periods ahead | 2, 3 | in-model | `forecast` param; SES is flat, so all periods sit at the final level |
| 10 | Fitted-vs-observed table with residuals | 3 | in-model | `output=csv` (`index,value,smoothed,error`) and the JSON `values`/`smoothed` arrays |
| 11 | Forecast error metrics | 2, 3 | in-model | JSON `errors`: `sse`, `mse`, `rmse`, `mae`, `mape` (one-step-ahead) |
| 12 | Chart of raw vs smoothed | 3 | in-model | `output=svg` (self-contained chart, gaps break the raw polyline, forecast dashed) |
| 13 | CSV export of the result table | 3 | in-model | `output=csv` |
| 14 | Free-form data entry (one value per line / delimited list) | 2, 3 | in-model | parser accepts commas, spaces, tabs, semicolons, newlines, JSON arrays; skips a text header |
| 15 | Optional user-supplied initial forecast value | 2 | **out-of-model (deliberate)** | see below |
| 16 | Double exponential smoothing (linear trend) | 3 | **out-of-model** | see below |
| 17 | Triple exponential smoothing / Holt-Winters seasonality | 3 | **out-of-model** | see below |
| 18 | Forecast confidence bounds | 3 | **out-of-model** | see below |
| 19 | Residual diagnostics (ACF, periodogram, Q-Q) | 3 | **out-of-model** | see below |
| 20 | Custom period labels (months, custom names) | 2 | **out-of-model** | see below |

## Out-of-model decisions (listed, not built)

- **User-supplied initial forecast (#15).** Both supported seeds are already reachable: `adjust=false`
  seeds at the first observation (the textbook SES default), and `adjust=true` divides by the partial
  weight sum, which is the bias-corrected alternative. An arbitrary third seed is a niche
  reproduce-my-spreadsheet knob that would add a param without adding a capability, and it interacts
  badly with `mode=auto` (the fitted alpha would depend on an arbitrary constant). Not built.
- **Double / triple exponential smoothing (#16, #17).** Holt's linear trend and Holt-Winters
  seasonality are different models with their own beta/gamma parameters, seasonal-period selection,
  and additive/multiplicative variants. They belong in their own tool, not as a mode on a single-
  exponential-smoothing block — folding them in would make every param conditional on `mode` and
  triple the surface. Explicitly stated as a limit in the page FAQ so the scope is honest.
- **Forecast confidence bounds (#18).** Prediction intervals require a state-space/ETS error model
  (or a residual bootstrap) plus a distributional assumption; reporting naive intervals from the
  one-step residual variance would be misleading for a multi-step flat forecast. Deferred with #16/#17,
  since the useful version of this ships with the ETS model.
- **Residual diagnostics (#19).** ACF, periodogram and Q-Q plots are a general time-series-diagnostics
  tool, not part of smoothing. The per-period `error` column is exported in CSV so these can be run
  elsewhere on the output.
- **Custom period labels (#20).** Presentation-only: the tool indexes periods from 1 and echoes the
  input order, so labels can be re-attached by the caller. Adding a parallel label series would double
  the input surface for no computational gain.

## Deltas we ship that the scanned tools do not

- All four decay conventions are accepted **and** all four equivalents are reported back, so a span
  can be read as a half-life without a second calculation.
- `mode=auto` fits alpha and reports it alongside the error metrics, which the hosted SES calculators
  either fix by hand (#2) or bury inside a fitted-model summary (#3).
- Gaps are first-class in a hosted tool: gap tokens parse, the level carries forward, gaps are skipped
  when scoring, and `ignore_na` switches the weighting basis.
- Three output shapes (JSON report, CSV table, SVG chart) from one run, all rendered locally.

## UX control patterns adopted

- **Sliders** for `alpha`, `span`, `halflife`, `com` and `forecast` — every scanned hosted tool exposes
  the smoothing constant as a directly manipulated value, and the decay parameters are exactly the
  "drag and watch the curve" case `kind = "slider"` exists for.
- **Friendly enum labels** (`[input.labels]`) on `mode` and `output`, so the decay conventions read as
  words rather than as raw slugs.
- **Preset chips** (`[[example]]`) covering the four realistic entry points: a plain alpha JSON run, a
  12-period finance-style EMA with `adjust=false` in CSV, an auto-fit run with a forecast horizon, and
  a half-life SVG chart over a series containing gaps. The hosted calculators all ship a pre-filled
  sample dataset; chips are the equivalent that also demonstrates the non-default switches.
- **Multiline series field** with a column-shaped placeholder, matching the paste-a-column entry both
  hosted tools use.

## Verification note

Everything in the "in-model" column is exercised by the block's unit tests and by the CLI
advertised-values matrix (one real run per `mode` and per `output` value, plus non-default `adjust`
and `ignore_na`), and the page spec asserts real rendered output plus a `?param=` deep link.

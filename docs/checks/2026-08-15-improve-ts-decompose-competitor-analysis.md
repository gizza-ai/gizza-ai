# ts-decompose — competitor analysis (2026-08-15)

Scan run **before** implementation, per `/improve-tool` Phase 2. All findings are paraphrased
observations of publicly visible feature/parameter lists. **No competitor copy, branding, or
trademark text was copied into this repo**; out-of-model items are listed here, not built.

## Scope + duplicate check

The backlog row is "Decompose a time series into trend, seasonal, and residual components and
plot them." (type hint `pure`, no heavy dependency needed).

Nearest existing blocks were checked before building:

- `blocks/moving-average` — smooths a series (simple/weighted/exponential windows). It produces
  *one* smoothed series, not a trend/seasonal/residual split, and has no seasonal period concept.
- `blocks/exponential-smoother` — single/double/triple exponential smoothing for **forecasting**;
  its Holt-Winters path models seasonality but emits fitted/forecast values, not an explicit
  three-component decomposition table or a component panel plot.
- `blocks/spline-smoother` — cubic smoothing splines over noisy numeric data; again one smoothed
  curve, no seasonality.
- `blocks/time-series-resample` — changes sampling frequency (upsample/downsample/aggregate).
- `blocks/fft-analyzer` — frequency-domain magnitude spectrum, not a time-domain decomposition.
- `blocks/least-squares-regression`, `blocks/multiple-regression` — parametric fits, no seasonal
  component and no residual panel plot.
- `blocks/stl-repair` — unrelated: 3D **STL mesh** repair, not seasonal-trend-loess.

No existing block decomposes a series into trend + seasonal + residual (grep for `seasonal`
across `blocks/*/core/src/lib.rs` returned nothing). The backlog row's own justification says the
same. Not a duplicate — built.

## Competitors reviewed

| # | Tool | What it offers (paraphrased) |
|---|------|------------------------------|
| 1 | statsmodels `tsa.seasonal.seasonal_decompose` (Python) | The reference **classical** decomposition. Params: series (needs ≥2 full cycles), `model` additive (default) or multiplicative, `period` (integer, required when the index carries no frequency), `filt` (custom moving-average coefficients), `two_sided` (centred MA by default, else trailing/past-only), `extrapolate_trend` (0 by default, so the trend and residual carry NaN at both ends; >0 or `"period"` fills them by linear least-squares extrapolation). Returns trend, seasonal, resid components satisfying `Y = T + S + e` (additive) or `Y = T·S·e` (multiplicative), plus a stacked four-panel plot helper. |
| 2 | statsmodels `tsa.seasonal.STL` (Python) | The reference **STL** (seasonal-trend decomposition by loess). Params: `period`, `seasonal` (odd length of the seasonal loess smoother, default 7), `trend` (odd length of the trend smoother; auto-derived from period and the seasonal window when unset), `low_pass` (odd, defaults to the smallest odd integer above the period), `seasonal_deg`/`trend_deg`/`low_pass_deg` (0 or 1, default 1), `robust` (off by default; switches on outlier-downweighting via an outer loop), `seasonal_jump`/`trend_jump`/`low_pass_jump` (interpolation-step speedups), and `fit(inner_iter, outer_iter)`. Returns the same trend/seasonal/residual triple. |
| 3 | planetcalc.com "Time Series Decomposition" (online calculator) | Browser calculator. Inputs: a time/value pair table (typed row by row or imported from CSV), a **seasonality size** (period) field, a **decimal-places** setting, and a model dropdown (additive vs multiplicative). Outputs: the decomposed table (trend, seasonal, random columns) plus separate seasonal-component and random-component charts, downloadable result files, rows-per-page paging (5/10/20/50/100/1000), and share/embed links. |
| 4 | metricgate.com "Seasonality & Trend Decomposition Calculator" (online) | Runs STL and a Hodrick-Prescott filter side by side. Inputs: dataset + date column (parses common date formats and auto-sorts) + numeric value column; a **sampling-frequency preset** (monthly 12, quarterly 4, weekly 52, daily 252, yearly 1); an STL **season window** that is either the literal `periodic` (default, fixed seasonal shape) or an odd integer (lets the seasonal shape drift); and an HP λ that auto-scales from the frequency or is typed in. Outputs: a four-panel STL plot (data, seasonal, trend, remainder), a components table, an HP trend overlaid on the original series, and a trend-overlay chart. UX: CSV/Excel import, a **Load-example** button, column mapping, checkbox-selected outputs, a Run button. |

Additional reference read for defaults/conventions: R's `stats::decompose` / `stats::stl`
(`s.window = "periodic"`, robust flag, four-panel plot) and the fpp3 textbook definitions of the
**strength of trend / strength of seasonality** measures.

## Table stakes → decision

Every table-stake below lands in the descriptor or in the out-of-model list. Nothing dropped.

### In-model — built into the descriptor

| Table stake | Param(s) |
|---|---|
| Paste a series in any common separator; tolerate a header row | `data` (newline/comma/tab/semicolon/space; header skipped) |
| `label,value` (date,value) rows, as every calculator's two-column table | `data` accepts `label,value` pairs; labels drive the x-axis ticks and the table/CSV/JSON `label` field |
| Classical decomposition (the planetcalc/`seasonal_decompose` engine) | `method = classical` |
| STL / loess decomposition (the metricgate/`STL` engine) | `method = stl` (default) |
| Additive vs multiplicative model | `model = additive` \| `multiplicative` (multiplicative decomposes in log space, then exponentiates) |
| Seasonal period, entered directly | `period` (2–1000) |
| Frequency presets (monthly 12, quarterly 4, weekly 7/52, daily 252) | `period` + one-click `[[example]]` chips for the common frequencies |
| Period **auto-detection** (neither Python API offers it; metricgate infers from a frequency dropdown) | `period = 0` (default) auto-detects via the autocorrelation peak |
| Centred vs trailing moving average | `two_sided` (classical) |
| Trend extrapolation so the ends are not blank | `extrapolate_trend` (default on; competitors default to NaN ends, which reads as a broken chart) |
| STL seasonal window, incl. the `periodic` fixed-shape default | `seasonal_window` (0 = periodic, else an odd length) |
| STL trend window | `trend_window` (0 = auto, using the standard `1.5·period/(1−1.5/n_s)` rule) |
| Robust (outlier-downweighting) STL | `robust` |
| STL inner/outer iteration control | `robust` selects the standard iteration counts (1 inner / 15 outer robust, 2 inner / 0 outer non-robust) — see out-of-model for exposing the raw counts |
| Four-panel plot: observed, trend, seasonal, residual | `output = svg` (the default) |
| Trend overlaid on the observed series | `trend_overlay` (default on) |
| Seasonally adjusted series (`Y − S` / `Y ÷ S`) | `show_adjusted` overlays it; it is a column in `table`/`csv`/`json` |
| Residual panel drawn as bars vs a line | `residual_style` = `bar` \| `line` |
| Components table with per-point values | `output = table` |
| Machine-readable export | `output = csv`, `output = json` |
| Decimal-places control (planetcalc exposes this explicitly) | `precision` (0–12) |
| Chart title and axis labels | `title`, `x_label`, `y_label` |
| Chart size, colour, light/dark theme, gridlines | `width`, `height`, `color`, `theme`, `grid` |
| Seasonal indices (average effect per position in the cycle) | reported in `table` and `json` |
| Strength of trend / strength of seasonality diagnostics | reported in `table` and `json` (fpp3 definition, `1 − Var(R)/Var(T+R)`) |
| Worked example / load-example button | five `[[example]]` preset chips (monthly additive, multiplicative sales, weekly period 7, classical + trailing MA, CSV export) |

### Out-of-model — listed, not built

| Feature | Why it is out of model |
|---|---|
| CSV/Excel **file upload** with a column-mapping UI (planetcalc, metricgate) | The page's pure-tool surface is a text field, not a file input; users paste the column. `blocks/csv-select-columns` covers extracting one column beforehand. |
| Hodrick-Prescott filter alongside STL (metricgate) | A different estimator (a penalised-smoothing trend/cycle split) with its own λ semantics — a separate tool, not a mode of a trend/seasonal/residual decomposition. |
| Real **date-axis** arithmetic — parsing dates, inferring the sampling frequency from them, and resampling irregular timestamps | Labels are carried through verbatim as tick text; the decomposition itself assumes evenly spaced observations, which is what both Python APIs assume too. `blocks/time-series-resample` handles regularising an irregular series first. |
| Multiple series / 2-D input (`seasonal_decompose` accepts a column matrix) | The page and chat surfaces take one pasted series; decomposing several at once would need a multi-series input shape this repo's pure-tool form does not have. |
| MSTL / multi-seasonal decomposition (several periods at once) | Out of scope for the backlog row and a materially different algorithm; a candidate for its own block. |
| PNG/PDF export, share links, embeds, result-file downloads, paging (planetcalc, metricgate) | Platform features, not tool logic. The page already offers Copy result and a Download link for text output; the SVG is standalone markup a user can save directly. |
| `filt` (arbitrary user-supplied moving-average coefficients) | An escape hatch for hand-built filters with no page-friendly control; `two_sided` covers the choice real users make. |
| Raw `inner_iter`/`outer_iter`, `*_deg` and `*_jump` STL knobs | Deep-internals tuning; the standard degree-1 loess and the standard iteration counts are used, with `robust` as the one switch that actually changes results. The jump parameters are pure speed hacks that trade exactness for time — the block runs the exact (jump = 1) path. |
| Forecasting from the fitted components | Already this repo's `blocks/exponential-smoother` (Holt-Winters). |

## Verification notes

- `method` × `model` matrix, both `two_sided` states, `robust` on/off, auto vs explicit `period`,
  and every `output` value are exercised end-to-end (unit tests + the page spec's wasm matrix),
  not just at the argv level.
- Correctness anchors: a synthetic series built as a known trend + a known repeating seasonal
  pattern is decomposed back to those components within tolerance, and the identity
  `observed = trend + seasonal + residual` (additive) / `trend × seasonal × residual`
  (multiplicative) is asserted point-by-point.

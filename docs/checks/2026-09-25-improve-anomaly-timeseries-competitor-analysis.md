# anomaly-timeseries — competitor scan + build decisions (2026-09-25)

Concise top-3 scan run at build time (new tool, so the "improve" pass is folded into the first
build). Everything below is **paraphrased** — no competitor copy, branding, or trademarks were
reused.

## Duplicate check (ran first)

`ls blocks/ | grep -iE 'anomal|outlier|zscore|timeseries|spike|seasonal'` →
`downsample-timeseries`, `iqr-outlier-trimmer`, `isolation-forest-anomaly`, `outlier-detector`,
`time-series-forecaster`, `time-series-generator`, `time-series-resample`, `ts-decompose`,
`z-score-calculator`, `z-score-normalize`. Verdict: **not a duplicate.**

| existing block | what it does | why it is not this row |
| --- | --- | --- |
| `outlier-detector` | GLOBAL z-score / modified z / Tukey fences over an unordered list (`numbers`, `z_threshold`, `iqr_k`) | no window, no ordering, no seasonality — a trending or seasonal series makes its global mean/SD meaningless |
| `z-score-calculator` | z of one value against a given mean/SD, or standardising a list | single-point statistic, not a detector |
| `iqr-outlier-trimmer` | removes/clips outlier ROWS of a CSV column | trimming, not per-timestamp flagging |
| `isolation-forest-anomaly` | multivariate isolation forest over a table of observations | row-wise, unordered, tree ensemble; no rolling baseline |
| `ts-decompose` | splits a series into trend/seasonal/residual (+ plot) | emits components, flags nothing; no threshold, severity, or anomaly list |
| `time-series-forecaster` | Holt-Winters projection forward | forecasts, does not score history |

The genuine delta this row asks for — a **rolling (windowed, leave-one-out) baseline** plus a
**same-phase seasonal rule**, with per-point scores, severity and an anomaly list — exists nowhere
in the repo.

## Competitors

### 1. DataHub Pro — Anomaly Detector (browser-local, no signup)
- **Closest competitor**: rolling z-score in the browser, paste-a-column input.
- Controls: rolling window (default 12), z threshold (default ~2.5), one method (rolling z-score).
- Input: one number per line or single-column CSV; strips headers, currency symbols, thousands
  separators and non-numeric rows.
- Output: per-period row with actual value, expected value (rolling mean), |z|, severity label
  (normal / warning ≥2σ / critical ≥3σ); summary counts of points scanned, critical, warning and
  an anomaly rate; CSV download and shareable link.
- Stated limits: false positives on strongly seasonal data (recommends deseasonalising first),
  on structural breaks until the window catches up, and on skewed distributions; no multivariate
  detection.

### 2. VictoriaMetrics — anomaly-detection built-in models (docs)
- Vocabulary reference for the non-ML model family: online z-score, online MAD, online seasonal
  quantile, rolling quantile.
- Parameters worth copying as *concepts*: `z_threshold`, a warm-up minimum sample count before
  scoring, seasonal interval, and a **detection direction** (flag only above / only below / both).
- Output shape: an anomaly score plus expected value and lower/upper bounds per point.

### 3. Practitioner guides (Tinybird / TigerData / MCP Analytics articles on statistical
time-series anomaly detection)
- Consensus table-stakes rules: rolling mean ± k·SD, robust median/MAD instead of mean/SD when the
  baseline itself contains spikes, and "compare each point to the same phase of previous cycles"
  for seasonal metrics.
- Repeated warnings: the SD is inflated by the very anomalies being hunted (→ MAD arm), a single
  threshold on a near-flat baseline fires on noise (→ an absolute tolerance deadband), and
  |z| ≥ 3 is the usual production cutoff with 2 as a watch level.

## Decisions taken into the build (in-model)

| capability | decision |
| --- | --- |
| numeric series input | `series`, family parser shared in spirit with `ts-decompose`: one number per line, `label,value` rows (dates keep their labels), or a single separated line; header rows skipped; `_` stripped |
| rule enum | `method` = `rolling_z` (default), `rolling_mad` (robust median/MAD), `seasonal_z` (same phase of other cycles), `combined` (worst of the rolling and seasonal rules, with a `rule` column saying which fired) |
| rolling window | `window` (default 12, 2–1000) + `min_periods` (0 = require the full window) |
| threshold | `threshold` (default 3) flags an anomaly; `warn_threshold` (default 2) labels near-misses as `warning` without flagging them |
| seasonal period + tolerance | `period` (default 7, 2–1000) for the seasonal rules; `tolerance` (default 0) is an absolute deadband — a point within it of its expected value is never flagged however large its score |
| detection direction | `direction` = `both` \| `above` \| `below` |
| centred vs causal | `center` (default off = only past points feed the baseline, matching live monitoring; on = a symmetric retrospective window and all other cycles for the seasonal rule) |
| output marking | per-point `index` (1-based), `label`, `value`, `expected`, `deviation`, `score`, `lower`/`upper` bound, `severity`, `anomaly`, `rule`; plus `anomaly_indices`, `anomaly_values`, `anomaly_scores` and a `summary` with counts, anomaly rate and the worst point |
| output formats | `output` = `json` \| `table` \| `csv`, plus `only_anomalies` to keep just the warning/critical rows |
| leave-one-out baselines | a point is always excluded from its own baseline, so a spike cannot hide inside its own mean (competitor tools that include the point under test under-report single large spikes) |
| flat-baseline honesty | zero spread gives no finite score: the point is reported with a `null` score and flagged critical only if it clears the tolerance, and `summary.flat_baseline` counts those |

## Considered, not built (out of model or deliberately declined)

- **Live database/metric connectors, alerting, webhooks, alert-suppression backoff** — need a
  server and state; gizza blocks are one-shot and browser-local.
- **Shareable result links / CSV download** — `output = csv` gives the same bytes; page links and
  downloads are the page runtime's generic job, not per-tool state.
- **ML detectors (isolation forest, autoencoders, Prophet)** — out of scope for this row by
  instruction, and the forest arm already ships as `blocks/isolation-forest-anomaly`.
- **Automatic period detection** — declined (not rejected as impossible): `ts-decompose` already
  owns `detect_period`, and silently guessing a period changes which points are called anomalies.
  `period` stays explicit so the verdict is reproducible.
- **Multivariate / correlated-metric anomalies** — a different input shape (a table), already
  served by `blocks/isolation-forest-anomaly`.
- **STL-residual detection** — deliberately left to `ts-decompose` + this tool in sequence rather
  than embedding a second decomposition engine here.

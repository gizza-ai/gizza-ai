# time-series-generator — competitor analysis (2026-08-16)

Scan run BEFORE implementation, so the shipped descriptor could absorb the table stakes in one
pass rather than as a later improve round. Five real competitors were profiled: one direct
browser tool, three Python libraries that define what practitioners expect from "synthetic
time series", and one general online test-data generator that people currently bend into the
job. **No competitor copy, wording, branding or assets were reused** — features and UX
patterns only, described here in our own words.

## Competitor profiles (paraphrased)

### 1. CalcBE "Synthetic Time Series Generator" (browser tool) — the direct competitor
The only close analogue that runs client-side with no account.

- **Index:** start timestamp, frequency limited to a fixed list (1 min / 5 min / 1 hour /
  1 day), point count, decimals.
- **Signal:** base value; linear trend with a per-step slope; one seasonality that is either a
  sine wave (amplitude + daily/weekly cycle) or an explicit 7-number day-of-week pattern.
- **Noise:** Gaussian (sigma) or AR(1) (phi + sigma).
- **Data-quality injection:** missing rate, outlier rate, explicit missing *blocks*
  (startIndex + length), outlier mode (additive spike / multiplicative spike / fixed
  magnitude), outlier direction (both / up only / down only).
- **Reproducibility:** either a CSPRNG or a seeded mode, with an option to put the seed in the
  share URL.
- **Output:** CSV, JSON, plus a "full profile" JSON that round-trips every setting.
- **UX:** three presets (web traffic hourly, sales daily, sensor minutely), line-chart preview,
  first-20-rows table preview, copy-settings-URL, download, import profile, clear.
- **Positioning:** explicitly "for testing, learning and demos", local-only, no upload.

### 2. Nike `timeseries-generator` (Python library)
- Composition model is **multiplicative with additive noise**:
  `ts = base_value * factor1 * … * factorN + noise`.
- Factor catalogue: `LinearTrend` (slope + intercept), `WhiteNoise` (stdev factor),
  `WeekendTrendComponents` (weekend vs weekday level), `HolidayTrendComponents` and
  `BlackFridaySaleComponents` (country-aware calendar spikes), `CountryYearlyTrend` (GDP-driven
  yearly drift), `EUEcoTrendComponents`, `ProductSeasonTrendComponents` (temperature-driven
  seasonality), `FeatureRandFactorComponents` (per-store/per-product level shifts).
- Output is a DataFrame that keeps **each factor's contribution as its own column** next to the
  final value — i.e. the decomposition is inspectable, not just the sum.

### 3. TimeSynth (Python library)
- Separates a **signal** from a **noise** process and lets you mix any pair.
- Signals: sinusoidal (frequency + amplitude), pseudo-periodic (frequency and amplitude
  themselves jitter), autoregressive, continuous autoregressive (CAR), NARMA, Gaussian process.
- Noise: white/Gaussian and red (correlated) noise.
- Supports **irregular sampling** (drop or jitter the time index), which is its main
  differentiator over spreadsheet-style tools.

### 4. `zaman` / generic pandas+numpy recipes (the "just write it yourself" baseline)
- The canonical recipe practitioners reach for: `pd.date_range(start, periods, freq)` for the
  index, then `level + slope*t + amplitude*sin(2*pi*t/period) + np.random.normal(0, sigma)`.
- What people always add by hand afterwards: clamping to non-negative for counts, rounding to
  integers, multiple seasonal periods at once (daily *and* weekly for hourly data), and a
  second column so joins/merges can be tested.
- Cost to the user: an environment, a notebook, and ~15 lines every time.

### 5. Mockaroo (online test-data generator)
- Not a time-series tool, but the tool people currently misuse for the job: a Date field plus a
  Formula field with `field('x') + days(n)` arithmetic, row count, and export to
  CSV/JSON/SQL/Excel.
- Strengths worth noting: many export formats, per-field null percentage ("blank" %), and
  schema save/share.
- Weakness that defines our opening: producing a *sequential* index needs formula tricks, and
  there is no concept of trend, seasonality or autocorrelated noise at all — every row is drawn
  independently.

## Gap analysis vs. what we shipped

### In-model, built in this pass

| Gap (source) | What we shipped |
| --- | --- |
| Linear trend only (1, 2, 4) | `trend` = `none` / `linear` / `exponential` / `logistic` / `random-walk`, with one `trend_strength` whose meaning is documented per mode |
| One seasonal cycle (1, 4) | `period` and `amplitude` take **comma-separated lists**, so `period="24, 168"` + `amplitude="8, 4"` superimposes a daily and a weekly cycle without new params |
| Sine or day-of-week only (1) | `seasonality` = `none`/`sine`/`cosine`/`square`/`triangle`/`sawtooth`/`weekday`, plus `weekday_pattern` (7 Mon–Sun numbers, mean-centred so it works additively too) |
| Fixed frequency list (1) | `interval` accepts any `<n><unit>` with `ms, s, m, h, d, w, mo, q, y` — calendar-correct months/quarters/years, matching the `time-series-resample` vocabulary |
| Additive composition only (1, 4); multiplicative-only (2) | `combine` = `additive` or `multiplicative` — in multiplicative mode `amplitude`/`noise_level` are read as fractions of the level |
| Gaussian/AR(1) noise (1, 3) | `noise` = `none`/`gaussian`/`uniform`/`ar1` with `noise_level` and `noise_phi` |
| Outlier rate/direction (1) | `outlier_rate`, `outlier_magnitude` (fraction of the value at that point), `outlier_direction` = `both`/`up`/`down` |
| Missing values (1, 5) | `missing_rate`, rendered as an empty cell in CSV/TSV and `null` in JSON/NDJSON |
| Clamping / integer counts (4) | `min_value`, `max_value` (blank = unbounded) and `decimals` (0 = whole numbers) |
| Single series only (1) | `series` = 1–20 parallel columns sharing the signal, each with its own noise stream |
| Seeded reproducibility (1, 3) | `seed`, a built-in SplitMix64 stream — identical rows in chat, CLI, page and tests |
| CSV/JSON export (1, 5) | `output` = `csv` / `tsv` / `json` / `ndjson` / `stats`, `header`, `labels` |
| Timestamp shape (1, 5) | `timestamp_format` = `auto` / `iso` / `date` / `epoch` / `index` |
| Presets (1) | Six `[[example]]` chips: daily sales, hourly web traffic with two cycles, IoT sensor with outliers, exponential growth, random walk, and a gappy/noisy QA fixture |
| Inspectable decomposition (2) | `output=stats` reports the achieved min/max/mean/sd/first/last plus the missing and outlier counts, so the generated series can be checked without leaving the tool |
| "Local, no upload" positioning (1) | Stated on the page and in the skill description |

### In-model, considered and rejected
- **Chart preview of the generated series (1).** Rejected for this pass: the shared page runtime
  renders text output, and a per-tool chart would mean a bespoke `custom.js`. `csv-chart-generator`
  already covers plotting a pasted series, so the composition is one paste away.
- **"Full profile" JSON import/export (1).** The page already encodes every field in the URL
  query string, and the generated CLI example is copy-pasteable — a second serialization format
  would be redundant state.
- **Explicit missing *blocks* by index (1).** Rejected as schema bloat versus `missing_rate`; a
  deliberate contiguous outage is better expressed by generating two series and concatenating.
- **Per-factor contribution columns (2).** Attractive, but it doubles the column count for every
  user to serve a debugging case that `output=stats` already covers.

### Out of model (not built, and why)
- **Country/holiday calendars, GDP trends, retail-event factors (2).** These need bundled
  reference datasets that would dwarf the wasm module; a browser-local tool should not ship a
  holiday database per country.
- **Gaussian-process, NARMA and CAR signals (3).** Real capability, but the parameterisation is
  research-grade and would not survive a nine-field web form; AR(1) covers the autocorrelation
  case that testers actually ask for.
- **Irregular / jittered sampling (3).** Deferred rather than refused — it interacts with the
  calendar-unit intervals in ways that need their own design pass.
- **API access, saved schemas, accounts, cloud batch (1, 5).** Out of model by construction:
  gizza tools are no-account and run entirely in the browser or the CLI.

## Sources
- <https://calcbe.com/en/tools/random/test-data/time-series-generator/>
- <https://github.com/Nike-Inc/timeseries-generator>
- <https://github.com/TimeSynth/TimeSynth>
- <https://www.index.dev/blog/generate-time-series-data-python>
- <https://www.mockaroo.com/help/formulas>

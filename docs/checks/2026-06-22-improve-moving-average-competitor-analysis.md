# moving-average — competitor analysis (2026-06-22)

## Tool

`gizza-ai/moving-average` — computes simple (SMA), exponential (EMA), and weighted
(WMA) moving averages over a numeric series, with a configurable look-back window
(`period`, default 3). Pure-Rust, runs on all surfaces (chat, CLI, standalone page).

## Top competitors surveyed

1. **GoodCalculators** — separate SMA and EMA calculators; data separated by line
   breaks/spaces/commas. https://goodcalculators.com/simple-moving-average-calculator/
   and /exponential-moving-average-calculator/
2. **JournalPlus** — SMA, EMA, and WMA for any period.
   https://journalplus.co/tools/moving-average-calculator/
3. **TrueCalculators** — SMA, WMA, EMA over a time series.
   https://truecalculators.net/statistics/moving-average-calculator/
4. **DevOven** — SMA, EMA (auto smoothing factor), WMA in one tool.
   https://www.devoven.com/tools/moving-average
5. **CME Group education** — reference for SMA vs EMA semantics (`k = 2/(N+1)`).
   https://www.cmegroup.com/education/courses/technical-analysis/understanding-moving-averages

## Capability diff (before improvements)

| Capability | gizza (initial) | Competitors |
|---|---|---|
| Simple moving average (SMA) | yes | all |
| Exponential moving average (EMA) | yes | most (GoodCalc, JournalPlus, TrueCalc, DevOven) |
| **Weighted moving average (WMA)** | **no** | JournalPlus, TrueCalc, DevOven |
| Per-point output (full series) | yes | yes |
| Flexible separators (space/comma/semicolon/newline) | yes | yes (GoodCalc, etc.) |
| Configurable period | yes | yes |
| EMA smoothing factor shown | yes (`2/(period+1)`) | DevOven (auto) |
| Runs locally / private | yes (in-browser wasm) | varies (server-side forms) |

## Gaps ranked + actions

1. **WMA missing** (in-model, pure math) — the most common third average across
   competitors (JournalPlus, TrueCalculators, DevOven all offer it). **CLOSED**:
   added a linear-weighted moving average (weights `1..=period`, newest heaviest,
   `weight_sum = period*(period+1)/2`) to the core, chat schema, CLI, web, page,
   manifest, and tests. Output now returns `sma`, `ema`, **and `wma`** arrays.
2. **Copy/SEO** — added "weighted" / "WMA" to the page title, description, tags,
   hero, and the About copy so the page matches the search intent competitors rank
   for. **CLOSED.**
3. EMA smoothing-factor transparency — already present (`smoothing_factor` field);
   competitive parity. No action.

## Out-of-model / deliberately not built

- **Charting / plotting** the averages (competitors render a line chart) — the
  page surface renders text/media only, no client-side plotting library is wired
  in; out of scope for a pure-compute block.
- **Custom EMA seeding modes** (first-value seed vs SMA seed) — gizza uses the
  standard SMA-seed convention; a configurable mode is low value and would expand
  the schema for an edge case.

## Verification (post-improvement)

- `cargo test --workspace` — 11 tests pass (10 core+block logic incl. WMA, 1 drift guard).
- `wafer build` — chat `block.wasm` validates/instantiates (317 KiB).
- CLI: `gizza tool moving-average series="1, 2, 3, 4, 5" period=3` →
  `{"count":5,"ema":[null,null,2.0,3.0,4.0],"period":3,"sma":[null,null,2.0,3.0,4.0],"smoothing_factor":0.5,"wma":[null,null,2.333333,3.333333,4.333333]}`.
- Page: Playwright `tool-page-moving-average.spec.ts` passes (SMA/EMA/WMA rendered as JSON).

## Sources

- https://goodcalculators.com/simple-moving-average-calculator/
- https://goodcalculators.com/exponential-moving-average-calculator/
- https://journalplus.co/tools/moving-average-calculator/
- https://truecalculators.net/statistics/moving-average-calculator/
- https://www.devoven.com/tools/moving-average
- https://www.cmegroup.com/education/courses/technical-analysis/understanding-moving-averages

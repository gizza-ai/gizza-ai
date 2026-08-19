# drawdown-analyzer — competitor analysis (2026-08-08)

Scan run BEFORE implementing, so the descriptor could be designed around the table stakes.
All notes are **paraphrased observations of behaviour** — no competitor copy, branding, or
trademarks are reproduced, and nothing from their pages is reused.

## Competitors reviewed

| # | Tool (function) | Reachable | Shape |
|---|---|---|---|
| 1 | Loris Tools — max drawdown calculator | yes | paste an equity series, metric cards |
| 2 | Foliolytic — max drawdown analyzer | yes | broker-CSV / reconstructed portfolio, underwater chart |
| 3 | MetricGate — drawdown analysis | yes | price-or-returns series + date column, chart + top-5 table |
| 4 | Omni Calculator — maximum drawdown | yes | two numeric fields (peak, trough) + CAGR recovery add-on |

A fifth candidate (a CFA-branded finance site's drawdown calculator) returned HTTP 403 to any
fetch, so it was replaced by Omni Calculator rather than running the scan with fewer sources.

## Observed feature set (paraphrased)

**Loris Tools** — single text field of comma-separated equity/balance values; echoes the parsed
point count; reports max drawdown %, peak value, trough value, peak→trough positions in the
sequence, current drawdown from the running peak, and total return. No chart, no dropdowns, no
presets. A worked default series is pre-filled in the field.

**Foliolytic** — ingests brokerage CSV exports (several brokers, auto-detected) and reconstructs a
daily portfolio against price history for a large ticker universe. Reports max drawdown, drawdown
duration (peak→trough), recovery period (trough→prior peak), and frames every drawdown as
depth + duration + recovery. Draws an underwater chart shaded by depth, names exact peak/trough
dates, and surrounds the drawdown numbers with a large suite of other risk metrics (Calmar, Ulcer,
Pain, Sharpe, Sortino, VaR). Also publishes a static reference table of well-known assets' max
drawdowns.

**MetricGate** — accepts either a price sequence or a periodic-returns sequence with an explicit
input-type toggle, plus a date column for alignment; converts prices to returns internally. Reports
max drawdown, recovery time, drawdown duration (defined as the whole underwater stretch: decline +
recovery), and average drawdown. Renders an underwater chart, a drawdown chart, and a **top-5
worst drawdowns** table, with checkboxes to enable/disable each output block.

**Omni Calculator** — two numeric inputs (peak value, lowest value after the peak); formula stated
openly as (trough − peak) ÷ peak; optional collapsible section that converts a drawdown into a
**time-to-recover in years given a CAGR** assumption; shows two worked examples with real index
and crypto numbers.

## Table stakes → where each one landed

| # | Table stake | Fit | Where it landed |
|---|---|---|---|
| 1 | Paste an equity/balance series | in-model | `series` (multiline field) |
| 2 | Prices **or** periodic returns, with an explicit toggle | in-model | `series_type` = `equity` \| `returns` (`Param::enumv`) |
| 3 | Percent-form returns (`1.2%`) as well as decimals | in-model | parser accepts both per value |
| 4 | Header row in a pasted column | in-model | `has_header` checkbox |
| 5 | Max drawdown % | in-model | `max_drawdown` |
| 6 | Peak value / trough value / their positions | in-model | per-episode peak & trough value + 1-based position |
| 7 | Drawdown duration (peak→trough) | in-model | `decline_periods` |
| 8 | Recovery time (trough→prior peak) | in-model | `recovery_periods` |
| 9 | Whole underwater stretch (decline + recovery) | in-model | `underwater_periods`, `longest_underwater_periods` |
| 10 | Current drawdown from the running peak | in-model | `current_drawdown` |
| 11 | Total return over the series | in-model | `total_return` |
| 12 | Underwater curve/chart | in-model (as text) | ASCII underwater plot in the page output + `underwater_curve` array in the JSON |
| 13 | Top-N worst drawdown episodes table | in-model | `top_n` (1–20, default 5) + ranked episode list |
| 14 | Average drawdown depth | in-model | `average_drawdown` |
| 15 | Exact peak/trough/recovery **dates** | in-model | dated `date,value` rows, or `start_date` + `frequency` |
| 16 | Ulcer index / pain index | in-model | `ulcer_index`, `pain_index` |
| 17 | Required gain to get back to the peak | in-model | `required_gain_to_recover` + per-episode `required_gain` |
| 18 | Time-to-recover in years at an assumed CAGR | in-model | `recovery_cagr` (0 = off) → `estimated_recovery_years` |
| 19 | Point count echoed back so you can check the paste | in-model | `count` on the first output line |
| 20 | Presets / a pre-filled worked series | in-model | three `[[example]]` preset chips |
| 21 | Friendly labels on the frequency/type selects | in-model | `[input.labels]` |
| 22 | Native date picker for the start date | in-model | `[[input]] kind = "date"` |

## Out-of-model (listed, not built)

- **Broker CSV auto-detection + portfolio reconstruction from a price database** (Foliolytic):
  needs a server, a multi-broker format registry, and a licensed price feed. A browser-local wasm
  tool has none of those. The in-model substitute shipped instead: dated `date,value` rows, which
  is what those pipelines ultimately produce.
- **Ticker lookup / "type SPY and get its drawdowns"**: needs a market-data backend.
- **Static reference table of famous assets' historical drawdowns**: that is licensed market data
  kept fresh server-side, not a computation.
- **Interactive SVG/canvas underwater chart with hover tooltips and zoom**: the page output surface
  is text, so the honest in-model answer is the fixed-width ASCII underwater plot (deterministic,
  copyable, and testable). A real interactive chart would need bespoke page JS; deferred rather
  than half-built.
- **Trading-calendar-aware dating with exchange holidays**: `frequency = trading` advances by
  weekdays (Mon–Fri) only — a holiday calendar is jurisdiction- and year-specific reference data.
  Stated as a limit on the page instead of being faked.
- **The wider risk-metric suite** (Sharpe, Sortino, Calmar, VaR): in-model, but it already exists in
  this repo as the `returns-risk-analyzer` block. Duplicating it here would be schema bloat; this
  tool stays the drawdown specialist.

## Considered, rejected

- **A minimum-depth threshold filter** (only count drawdowns deeper than X%): ranking by depth and
  showing the top N already suppresses the noise it would remove, and it would add a fourth numeric
  knob for the same outcome.
- **Two-field peak/trough mode** (Omni's shape): a degenerate case of the series input — a
  two-value series `276.21, 222.83` produces exactly that answer, and it is documented in the FAQ.
- **Enable/disable checkboxes per output block** (MetricGate): the whole report is a few lines of
  text that Copy-result already handles; toggles would add schema surface for no capability.

## Conventions we state that competitors mostly leave implicit

- Drawdown is measured against the **running peak of the series itself** (high-water mark), not a
  rolling window.
- An episode ends only when the series **closes back at or above its prior peak**; an episode still
  underwater at the last observation is reported as ongoing, never as recovered.
- `decline` = peak→trough, `recovery` = trough→prior-peak, `underwater` = the sum of both.
- The ulcer index is the RMS of the underwater curve over **every** observation, and the pain index
  is its mean absolute value — both including the periods at a new high (as zeroes).

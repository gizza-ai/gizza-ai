# Bollinger Bands — competitor analysis (2026-06-21)

## Tool

`bollinger-bands` — computes Bollinger Bands for a price/value series: rolling SMA
middle band with upper/lower bands at ±`num_std` (default 2) **population** standard
deviations over a window of `period` values (default 20). Also returns %B and
bandwidth for the latest point. Pure-Rust, runs on all three surfaces (chat/LLM API,
CLI, in-browser page).

## Surfaces verified

- **Chat block** — `wafer build` validated the wasm block (315.8 KiB); drift-guard
  schema test passes (no LLM-facing schema drift).
- **CLI** — `gizza tool bollinger-bands prices="1 2 3 4 5" period=5 num_std=2` →
  structured JSON with `bands`, `percent_b`, `bandwidth`. Default `period=20` /
  `num_std=2` confirmed; error paths (too few values, non-numeric) confirmed.
- **Page** — Playwright `tool-page-bollinger-bands.spec.ts` passes; query-param
  prefill + auto-run wired (`?prices=…&period=20&num_std=2`).

## Competitor landscape

| Source | What it offers | Notes |
| --- | --- | --- |
| StockCharts ChartSchool | Canonical definition: middle = 20-SMA, upper/lower = SMA ± 2·stddev; separate BandWidth + %B indicators | Reference for the formula. Charting platform, not a paste-a-series calculator. |
| Wikipedia / QuantInsti / Britannica | Educational formula pages; confirm **population** stddev (÷N), default (20, 2), %B = (price−lower)/(upper−lower) | No interactive calculator. |
| iFOREX / broker education pages | Conceptual explainers tied to a trading platform | Require an account / live chart; no standalone numeric tool. |
| Stock Indicators for .NET / QuestDB | Library/SQL implementations | Code, not an end-user tool. |
| Generic "online Bollinger Bands calculator" pages | Single-window calculator: enter a few prices, get one set of bands | Usually one window only, no %B/bandwidth, no rolling series, often ad-heavy and server-side. |

## Gap analysis (fit-to-model)

Capabilities already covered (at or above the typical online calculator):

- **Rolling bands over the whole series**, not just one window — most free calculators
  compute a single window; this returns one band per window with the source index.
- **%B** and **bandwidth** for the latest point — many calculators omit these.
- **Configurable period and multiplier** with sensible defaults (20, 2).
- **Population standard deviation** matching the canonical StockCharts/Wikipedia
  definition.
- **Three surfaces** (chat/CLI/page), all local/private, no upload, no account.

Out-of-model / intentionally not built:

- **Charting / plotting** the bands — gizza pages are text/number/media outputs; a
  rendered price+band chart would need a client-side plotting surface that the page
  framework doesn't provide. Listed, not built.
- **Live market-data fetch** (enter a ticker, auto-pull prices) — that is a network +
  market-data-API tool, out of scope for a pure-compute block; the user pastes the
  series.
- **EMA-based bands / other moving-average types** — the standard Bollinger definition
  is SMA-based; an EMA variant could be a future enhancement but is not the canonical
  indicator and was left out to keep the schema focused.

No competitor copy, branding, or trademarks were used. The implementation follows the
publicly documented mathematical definition only.

## Outcome

The tool meets or exceeds the free online Bollinger Bands calculators for in-model
(pure-compute) scope: rolling bands, %B, bandwidth, configurable period/multiplier,
canonical population stddev, on three private local surfaces. No in-model gaps remained
to close.

# candlestick-chart — competitor analysis (2026-06-22)

New tool: render OHLC (open/high/low/close) price data from CSV into an SVG
candlestick chart. Pure-Rust (no deps in core), so it runs on all backends
including the chat Service Worker. Surfaces: chat + CLI (no standalone page — a
pure-Rust image-bytes output has no page render mode, same as
`line-series-chart` / `scatter-chart` / `heatmap-chart`).

## Surfaces verified

- **chat / LLM API** — `wafer build` validates the chat `block.wasm` instantiates
  (OK, 334 KiB); schema single-sourced from `descriptor()` with a drift-guard
  test (`schema_json_matches_authored_chat_schema`).
- **CLI** — `gizza tool candlestick-chart data=… title=… trendline=…` renders a
  valid SVG: confirmed up-bars are green (`#16a34a`), down-bars red (`#dc2626`),
  background + one body rect per candle, date labels on first/last bar, and the
  optional dashed closing-price trendline (`#2563eb`) only present when
  `trendline=true`. Error paths verified (wrong column count, non-numeric,
  high < low, empty input).
- **page** — none (image-bytes output, by design).

## Top competitors surveyed

1. U2Tool Candlestick Chart Generator
2. ChartGen.ai (AI candlestick chart maker)
3. Formula Bot free candlestick chart maker
4. CleanChart (OHLC visualization guide + maker)
5. Image Online Graph Maker (HTML5 candlestick engine)

## Capability gap diff (fit-to-model)

Pure-Rust static-SVG model. Capabilities ranked by fit:

| Competitor capability | Status in this tool |
| --- | --- |
| OHLC input via CSV | Done — 4-col (O,H,L,C) or 5-col (label/date,O,H,L,C) |
| Auto column mapping / header skip | Done — leading non-numeric header row auto-skipped |
| Up/down color coding | Done — green (close>=open) / red (close<open) bodies + wicks |
| Price (y) axis labels | Done — min / mid / max with gridlines |
| Date/x-axis labels | Done — label shown on first and last candle |
| Chart title + sizing | Done — `title`, `width`, `height` params |
| **Closing-price trendline** ("Show Trendline") | **Added this pass** — optional `trendline` boolean overlays a dashed line through each bar's close |

### Out-of-model (intentionally not built; documented, not copied)

- **Interactivity / zoom / bidirectional scrolling** — the output is a static SVG;
  pan/zoom requires a live charting runtime (HighCharts-style), which the
  image-bytes model does not have.
- **Volume sub-pane / VWAP overlay** — would require an additional volume column
  and a second stacked plot; deferred to keep the OHLC schema simple. In-model in
  principle (pure SVG) but out of scope for the initial tool.
- **Candlestick pattern recognition** (doji / hammer / engulfing detection) — this
  is price-action *analysis*, not visualization; out of scope for a chart renderer.
- **AI/natural-language chart requests** — handled by the chat surface itself
  (the LLM calls the tool); not a tool-internal feature.

No competitor copy, branding, or trademarks were used; color choices and layout
follow the existing gizza chart-block house style (`line-series-chart`).

## Result

Built + improved (added the closing-price trendline gap). All tests pass (1 drift
+ 8 core), `wafer build` OK, CLI verified across happy + error + trendline paths,
generator renders 209 tools.

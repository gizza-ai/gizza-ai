# rsi-calculator — competitor analysis & surface checks (2026-06-29)

**Tool:** `rsi-calculator` — compute Wilder's Relative Strength Index from a pasted price series.

## Surface checks

| Surface | Check | Result |
| --- | --- | --- |
| Core/unit | `CARGO_BUILD_JOBS=1 cargo test --workspace` in `blocks/rsi-calculator` | ✅ 15 tests passed (schema drift + RSI math/errors) |
| Chat block | `CARGO_BUILD_JOBS=1 wafer build` in `blocks/rsi-calculator` | ✅ `target/block.wasm` validated |
| Web wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/rsi-calculator/web --target web --release --out-dir pkg` | ✅ `web/pkg` generated |
| CLI | `gizza tool rsi-calculator prices='1 2 1 2 1 2' period=2 overbought=60 oversold=40` | ✅ returned JSON with `latest_rsi: 68.75` and `latest_signal: overbought` |
| Page generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/rsi-calculator/` |
| Page | `xvfb-run npx playwright test tool-page-rsi-calculator.spec.ts` | ✅ 3 passed (Wilder sample, query params/custom thresholds, validation error) |

## Competitors reviewed

1. **MarketBeat RSI Calculator** — retrieves market history for a ticker and reports RSI/interpretation.
2. **Good Calculators Relative Strength Index Calculator** — paste closing prices separated by spaces, commas, or lines and calculate RSI.
3. **CalculatorBox RSI Calculator** — paste prices, Wilder smoothing, overbought/oversold zone copy.
4. **ToolDone RSI Indicator Tool** — momentum/overbought/oversold calculator with trading-focused explanations.
5. **QuantStock RSI Calculator** — educational RSI calculator and indicator description.

## Gap analysis

| Capability | Competitors | gizza `rsi-calculator` | Decision |
| --- | --- | --- | --- |
| Paste closing prices | Common among calculator-style competitors | ✅ spaces, commas, semicolons, tabs, newlines | Implemented parser with input limit |
| Wilder smoothing | Common / expected for RSI | ✅ seeded with simple average, then Wilder smoothing | Implemented and pinned with tests |
| Default 14-period RSI | Standard | ✅ default 14 | Implemented |
| Custom RSI period | Common | ✅ integer `period`, 1..10000 | Implemented |
| Overbought/oversold classification | Common copy in all competitors | ✅ configurable thresholds, latest signal | Implemented |
| Full per-point series | Some tools only show one value | ✅ returns RSI, avg gain, avg loss arrays | Stronger for downstream/API use |
| Ticker/history lookup | MarketBeat-style tools fetch data | ❌ no network fetch | Out of scope; gizza tool runs locally on user-supplied data |
| Chart visualization | Trading platforms show charts | ❌ JSON/text output | Out of model for current page generator; data is easy to chart externally |

## Improvements made from analysis

- Added page copy and metadata around Wilder RSI, local execution, and price input formats.
- Exposed period and threshold controls instead of hard-coding only the 14/70/30 defaults.
- Returned both per-point RSI and average gain/loss arrays so users can inspect warm-up and smoothing behavior.
- Added page tests for query-param deep-linking and insufficient-data errors.

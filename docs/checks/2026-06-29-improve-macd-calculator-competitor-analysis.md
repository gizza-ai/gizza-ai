# macd-calculator — competitor analysis & surface checks (2026-06-29)

**Tool:** `macd-calculator` — compute the Moving Average Convergence Divergence indicator from a local price series.

## Surface checks

| Surface | Check | Result |
| --- | --- | --- |
| Core/workspace tests | `cd blocks/macd-calculator && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 12 tests passed (descriptor drift guard + core MACD vectors/errors) |
| Chat block | `cd blocks/macd-calculator && CARGO_BUILD_JOBS=1 wafer build` | ✅ produced and validated `target/block.wasm` |
| Web wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/macd-calculator/web --target web --release --out-dir pkg` | ✅ built `web/pkg` |
| Generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `/tools/macd-calculator/` |
| CLI | `gizza tool macd-calculator prices='1,2,3,4,5,6' fast=2 slow=4 signal=2` | ✅ returned expected JSON with latest MACD/signal/histogram = `1.0/1.0/0.0` |
| Page | `cd tests && xvfb-run npx playwright test tool-page-macd-calculator.spec.ts` | ✅ 3 passed (custom periods, default 12/26/9, query-param deep-link) |

## Competitor scan

Searches reviewed:
- `MACD calculator online free top competitors TradingView StockCharts Investopedia CalculatorSoup`
- `online MACD calculator moving average convergence divergence calculator competitors`

Representative competitors and references:

1. **TradingView MACD / Ideas** — chart-native MACD indicator; public documentation states the standard formula `MACD = EMA(12) - EMA(26)` and a signal line over MACD.
2. **StockCharts.com** — charting suite with technical indicators and portfolio/watchlist workflows.
3. **Thaurus Moving Average Convergence Divergence calculator** — standalone MACD calculator-oriented page.
4. **Moomoo help center MACD documentation** — explains MACD as short/fast EMA minus long/slow EMA.
5. **Investopedia technical-analysis tooling overview** — market context for charting/indicator tools rather than a small standalone calculator.

## Gap / fit analysis

| Capability | Competitors | gizza `macd-calculator` | Decision |
| --- | --- | --- | --- |
| Standard formula | Most use fast EMA 12, slow EMA 26, signal EMA 9 | ✅ defaults to 12/26/9 and exposes all three periods | Built |
| Custom periods | Charting tools expose configurable periods | ✅ integer `fast`, `slow`, `signal`; validates fast < slow and period bounds | Built |
| Local pasted data | Standalone calculators accept user data; charting suites use tickers | ✅ accepts spaces/commas/semicolons/newlines, oldest-first | Built |
| Full series output | Charting tools show lines visually; calculators often show latest values | ✅ returns fast EMA, slow EMA, MACD line, signal line, histogram arrays plus latest values | Built |
| Privacy/offline | Web charting tools generally upload/use remote market data | ✅ pure wasm + CLI/chat surfaces, no network required | Built |
| Ticker lookup / live charts / alerts | TradingView/StockCharts provide market data, plotting, alerts | ❌ out-of-model: requires network data feeds and chart UI beyond this pure calculator | Not built |
| CSV upload / export | Some spreadsheet-style tools support file import/export | Partial: paste a price series; JSON output can be copied/downloaded by browser | Good enough for current model |

## Improvements made from analysis

- Used the industry-standard default periods: fast 12, slow 26, signal 9.
- Returned both latest values and complete aligned arrays so downstream users can plot the MACD/signal/histogram themselves.
- Added input validation for non-finite values, empty series, fast >= slow, oversized periods, and insufficient slow-period data.
- Documented local/private execution in page copy.

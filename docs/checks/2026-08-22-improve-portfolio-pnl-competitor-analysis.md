# portfolio-pnl competitor analysis — 2026-08-22

Tool: `portfolio-pnl` — calculate per-position and portfolio profit/loss from user-supplied entry and current prices.

## Competitor scan

Search query: `portfolio profit loss calculator positions entry price current price fees tax sort P&L tool`.

| Competitor/tool family | Table-stakes capabilities observed | In model for gizza? | Decision |
| --- | --- | --- | --- |
| Broker/trading profit-loss calculators such as IFCM-style instrument calculators | Entry/open price, current/close price, quantity or lot size, long/short direction, concrete P/L amount, percentage-style return, simple worked examples | Yes for user-supplied prices; no for broker-specific contract metadata | Support long and short rows, entry/current prices, quantities, cost basis, market value, P/L amount, and P/L percentage. Do not model broker-specific lots, leverage, swaps, or margin rules. |
| Crypto/futures calculators such as Binance Futures calculators | Long/short selection, entry and exit prices, position size, fee-rate assumptions, ROI/PnL, liquidation or margin-related extras | Partly | Support long/short, position size, fee percent, P/L amount, and return percent. Liquidation price, leverage, funding, and exchange-specific margin are out of model because this pure offline block has no exchange risk engine. |
| Position-size and lot-size calculators such as FundedNext-style tools | Entry price, account/currency context, risk and lot sizing controls, clear result tables, disclaimers | Partly | Include a currency prefix, spreadsheet paste flow, and clear report table. Risk-based position sizing is adjacent but not the requested P/L calculation, so it is left out. |
| Portfolio tracker / deposit-withdrawal P&L tools | Multiple rows, aggregate P/L, winners/losers, fees, CSV import, history-based realized/unrealized totals | Partly | Support multi-row paste, totals, winners/losers, flat and percent fees, optional dividends/income, sorting, and tax estimate. Historical account import, live pricing, and realized-vs-unrealized tax lots are out of model. |

## UX controls to match

- Spreadsheet-style textarea for multiple positions: in model and implemented as the primary `positions` field.
- Long/short selector: in model and implemented as the `side` enum plus per-row side override.
- Fee and tax numeric controls: in model and implemented; page uses slider controls for percent fee and tax rate.
- Sort or ranking by gain/loss/value: in model and implemented via `sort` enum.
- Preset examples: in model and implemented as three example chips.
- Live price lookup, broker contract specs, leverage, liquidation, funding, and tax-lot accounting: out of model for this pure deterministic tool.

## Defaults and worked examples chosen

- Default side: `long`.
- Default fee percent: `0`.
- Default tax rate: `0` so tax lines appear only when requested.
- Default sort: input order to preserve spreadsheet order.
- Default currency: `$`, with a free-text prefix for other symbols or `USD `.
- Worked examples cover basic stock gains/losses, mixed long/short rows with fees, and crypto-style fractional quantities.

## Implementation notes

The descriptor includes every in-model table-stakes control above. The core intentionally requires the user to supply current prices and rejects negative prices; short exposure is represented by side or negative quantity. Results are formatted as deterministic text so CLI, chat, and browser page surfaces can assert exact output.

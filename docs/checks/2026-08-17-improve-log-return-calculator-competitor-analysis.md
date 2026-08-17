# log-return-calculator competitor analysis (2026-08-17)

Tool: `log-return-calculator` — turns a pasted price/level series into continuously-compounded (log) returns `ln(p_t / p_{t-1})`, with the simple return of each step, a running cumulative, the total, per-period and annualized volatility, annualized log and simple rates, and summary/table/CSV/JSON output.

Research method: WebSearch sweep for online log-return, continuously-compounded-return, and historical-volatility calculators, plus the tutorial/spreadsheet pattern that most users actually follow. All observations below are paraphrased behaviour and interface patterns; no competitor copy, naming, or styling was reused.

## Sources scanned (top 3 + background)

1. **Single-investment log return calculators** (the dominant shape, e.g. the rate-of-return-expert style page). Three inputs — initial value, final value, number of periods — and one output: `ln(V_f / V_i) / t`, shown as a percentage. No series input, no table, no volatility. Their help text stresses that the period unit you enter determines whether the rate is daily, monthly, or annual.
2. **Historical / annualized volatility calculators** (e.g. the Pineify-style free stock volatility page). Ticker-driven rather than paste-driven: pick a symbol and a lookback of 1 month to 5 years, and the tool fetches prices, computes daily `ln(close/prev close)`, takes the standard deviation, and annualizes by `√252`. Output is one headline volatility number plus, sometimes, an interpretation band. No per-row table, no CSV, no arbitrary pasted data.
3. **Total-return / CAGR calculators** (investment-return and stock-return calculator pages). Start value, end value, years, sometimes contributions or dividends; output total return and annualized return. They speak in simple-return and CAGR terms only — log returns are not offered, and there is no series input.

Background patterns, not standalone competitors: spreadsheet and Python tutorials (`=LN(B3/B2)` filled down; `np.log(prices / prices.shift(1))`). These set the mental model most users arrive with — a column of prices in, a column of log returns out — and they are the closest thing to a per-row table competitor, but they are instructions rather than a tool.

## Table-stakes capabilities and UX patterns

| Capability / UX pattern | Seen in competitors | In current gizza model? | Decision |
| --- | --- | --- | --- |
| Single-pair log return (`ln(end/start)`) | Log return calculators (all) | Yes | A 2-price series is the same computation; `total log return` is the headline line. |
| Divide by number of periods | Log return calculators | Yes, generalized | `mean log return` is the per-period figure; `annualized log return` scales it by periods-per-year. |
| Series (column) input | Only spreadsheet/Python tutorials | Yes | The core differentiator: paste up to 2,000 prices, one per line or one comma-separated line. |
| Per-row output table | Spreadsheet fill-down | Yes | Row per price, with `—` on the opening row because there is no prior price. |
| Cumulative / running total | Spreadsheet patterns | Yes | `cumulative log` column, demonstrating time-additivity visually. |
| Simple return shown alongside | Total-return calculators | Yes | Both conventions in one table so users can compare rather than convert by hand. |
| Standard deviation of log returns | Volatility calculators | Yes | Sample (n−1) standard deviation of the per-period log returns. |
| `√252` annualization | Volatility calculators | Yes, generalized | `periods_per_year` enum: 252, 365, 52, 26, 12, 4, 1. |
| CAGR / annualized simple rate | Total-return calculators | Yes | `exp(annualized log return) − 1`. |
| Best / worst period, win rate | Analytics dashboards | Yes | Best and worst step plus a positive/negative period count. |
| Percentage vs decimal display | Log return calculators show % only | Yes, both | `unit` enum; decimal exposes the raw natural-log value for modelling. |
| Decimal-place control | Rare; most fix 2 dp | Yes | `decimals` slider, 0–10. |
| Date/label column preserved | Ticker tools only (via their own data) | Yes | `label,price` rows keep dates; tab/comma/semicolon accepted. |
| Header-row tolerance for pasted exports | Rarely handled | Yes | `has_header` checkbox skips the first line. |
| Currency symbols and thousands separators | Rarely handled | Yes | Stripped during parsing; `1,234.50` stays one price. |
| CSV export for a spreadsheet | Rare on free pages | Yes | `output=csv`, numeric columns with no `%` or `+` so they stay numeric. |
| JSON output | Not seen | Yes | `output=json` for scripting and the CLI/chat surface. |
| Runs locally, no upload | Not typical (server-side or API-backed) | Yes | WebAssembly in the browser. |
| Ticker lookup / price download | Volatility and stock-return calculators | Out-of-model | Requires a market-data feed and a network call; conflicts with local-only compute. |
| Charts (price line, return histogram) | Some volatility pages | Out-of-model for this page | The page contract is text output; the table plus CSV covers the analysis path. |
| Dividend / contribution adjustment | Total-return calculators | Out-of-model | Belongs to a total-return tool; this one is a pure price-series transform. |
| Rolling-window volatility, EWMA, GARCH | Advanced/quant platforms | Out-of-model | Materially different tool; would need window controls and its own output shape. |
| Multi-series / correlation, beta, Sharpe | Portfolio analytics platforms | Out-of-model | Single-series scope keeps the parameter set small and the output legible. |
| Downsampling (daily → monthly resample) | Data platforms | Out-of-model | Needs real date arithmetic; the tool deliberately treats rows as evenly spaced. |
| Negative/zero-price tolerance | N/A — undefined everywhere | Out-of-model by definition | Rejected with the offending row number rather than emitting `NaN`/`-inf`. |

## Defaults and examples chosen

- `periods_per_year=252` — the near-universal default in volatility tooling for equity closes, and the value every `√252` tutorial assumes.
- `unit=percent` with `decimals=4` — competitors show percentages, but 2 dp loses too much on daily returns where moves are often under 1%; 4 dp keeps `+0.4021%` readable. `decimal` is offered because modelling workflows want the raw log value.
- `output=summary` — competitors give either one number or one column. The summary gives the headline block *and* the per-row table so a single run answers both the "what was the return" and the "show me the steps" questions.
- `has_header=false` — most pastes are a bare column; the checkbox is one click for the export case.
- Example chips cover the four realistic entry points: labelled daily closes → summary, a monthly NAV export with a header row → CSV, a one-line comma-separated series → decimal table, and the 50 → 70 → 50 symmetry demonstration that makes the log-vs-simple difference obvious.

## Copy and UX notes

- Competitor pages lean on the formula and stop. The page copy instead leads with *why* log returns exist (time-additive, symmetric) and shows the additivity in the worked example by summing the table's own column, which is the point that spreadsheet tutorials never make explicit.
- The prices-not-returns confusion is the most common misuse of these calculators and is called out first in the limits list.
- Every competitor is silent about zero/negative prices; here the error names the row and explains why the operation is undefined.
- The volatility FAQ documents the n−1 sample formula and the log-vs-simple choice, because that is the usual reason two tools disagree on the same series.
- "Educational only — not financial advice" appears in the hero, the copy, and the computed output note.

## Outcome

No in-model gaps found against the top 3. Every capability those tools expose is present and generalized to a series; the remaining competitor features (ticker download, charts, dividends, rolling/multi-series analytics) are out-of-model for a local, single-series, text-output tool.

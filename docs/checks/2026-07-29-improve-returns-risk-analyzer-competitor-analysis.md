# returns-risk-analyzer — competitor analysis (2026-07-29)

Function: compute annualized return/risk statistics from a periodic returns series: annualized return, volatility, Sharpe, Sortino, drawdown-style metrics, and best/worst periods. Pure compute; runs locally in browser / CLI / chat. Competitor details are paraphrased from public tool pages and finance calculator docs; no copy, branding, or trademarks reused.

## Competitors surveyed

| # | Tool / pattern | Input pattern | Metrics shown | Controls / UX | Notes |
|---|---|---|---|---|---|
| 1 | Sortino ratio calculators | returns or average return + target + downside deviation | Sortino, downside risk | target / MAR field, annualization explanation | Often narrow: Sortino only, not a full suite. |
| 2 | Risk-adjusted return calculators | returns or summary return/volatility fields | Sharpe, Sortino, Treynor/Information variants | risk-free rate, benchmark beta/return for some ratios | Some ratios need benchmark data that is outside this tool's input model. |
| 3 | Drawdown / portfolio risk calculators | pasted returns series | max drawdown, Sharpe, Sortino, volatility, sometimes VaR | frequency selector, chart/equity curve, export | Charts and VaR are useful but not required for v1 text output. |
| 4 | Spreadsheet / quant-library workflows | CSV column of returns | mean, stdev, CAGR, Sharpe, Sortino, drawdown | explicit formulas, sample vs population choices | Good precedent for stating statistical conventions. |

## Table-stakes → decision

| Capability | Decision |
|------------|----------|
| Paste a returns series | **IN** — `returns` required textarea, accepts newlines, commas, whitespace. |
| Accept decimals and percents | **IN** — `0.012` and `1.2%` supported per value. |
| Frequency / annualization | **IN** — `periods_per_year` enum: daily 252, weekly 52, biweekly 26, monthly 12, quarterly 4, annual 1. |
| Annual risk-free rate | **IN** — `risk_free_rate` percent for Sharpe. |
| Sortino target / MAR | **IN** — `target_return` percent for downside deviation and Sortino. |
| Annualized return and volatility | **IN** — geometric annualized return; sample stdev annualized by sqrt(periods). |
| Sharpe and Sortino ratios | **IN** — explicit undefined handling for zero volatility/no downside. |
| Max drawdown / Calmar | **IN** — table-stakes in broader drawdown calculators and cheap in-model. |
| Best/worst period and positive period share | **IN** — useful sanity checks from pasted series. |
| Header row skip | **IN** — `has_header` checkbox for spreadsheet copy-paste. |
| Equity curve / drawdown chart | **OUT** — current page surface is text-first; documented by providing max drawdown number. |
| VaR / CVaR | **OUT** — statistically meaningful VaR needs method choices and more assumptions; not in v1. |
| Benchmark ratios (Treynor / Information) | **OUT** — need benchmark/beta series inputs, beyond this one-series model. |
| Financial advice / portfolio recommendation | **OUT** — output is educational metrics only. |

## UX / page controls shipped

- Multiline `returns` textarea with decimal examples.
- `periods_per_year` select with labels for daily/weekly/biweekly/monthly/quarterly/annual.
- Numeric text fields for `risk_free_rate` and `target_return`, placeholders included.
- `has_header` checkbox for spreadsheet-pasted columns.
- Example chips for monthly risk-free and daily percent-return inputs.

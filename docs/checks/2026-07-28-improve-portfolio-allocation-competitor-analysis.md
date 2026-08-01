# Competitor analysis — portfolio-allocation (2026-07-28)

Tool function: break a list of portfolio holdings into percentage allocation by holding, asset class, sector, or account with chart-ready output. This is a pure local calculator; all competitor notes are paraphrased.

## Competitors reviewed

### 1. Portfolio Visualizer asset allocation tools
- Table-stakes: asset-class percentages, total portfolio value, rebalancing-style grouped output, readable percentages.
- UX patterns: example portfolios, dropdown grouping concepts, numeric tables with percent columns.
- Model fit: local percentage breakdown is in-model; historical returns and market data are out-of-model.

### 2. Sharesight / broker portfolio allocation reports
- Table-stakes: holdings grouped by sector, asset class, market/account, and chart-ready percentages.
- UX patterns: select a grouping dimension, largest slices first, exportable table.
- Model fit: grouped math is in-model; connected brokerage import, live quotes, and tax reports are out-of-model.

### 3. Empower / personal finance allocation dashboards
- Table-stakes: account and asset allocation summaries, diversification/concentration cues, pie-chart-friendly slices.
- UX patterns: account vs asset views, percentage bars, rollup of small holdings into a readable tail.
- Model fit: local grouping, Top-N tail folding, and a concentration score are in-model; account sync and recommendations are out-of-model.

## Decisions

| Table-stake | Decision | Where |
| --- | --- | --- |
| Paste holdings from spreadsheet/CSV | in-model | multiline `input` |
| Group by asset class | in-model | `group_by = asset` default |
| Group by holding, sector, account | in-model | `group_by` enum choices |
| Percent of total and total value | in-model | output table |
| Chart-ready label/value/percent rows | in-model | output lines per slice |
| Largest-first and alphabetical sorts | in-model | `sort` enum |
| Fold small slices into Other | in-model | `top_n` integer |
| Currency prefix | in-model | `currency` text input |
| Diversification/concentration cue | in-model | HHI score + label |
| Live market prices | out-of-model | requires price APIs/accounts |
| Automatic ticker sector lookup | out-of-model | requires securities database |
| Brokerage import/sync | out-of-model | requires user accounts and credentials |
| Investment advice/rebalancing recommendation | out-of-model | financial advice, not just calculation |

## UX matched

- `group_by` and `sort` are fixed choices with `Param::enumv` and friendly labels.
- `top_n` is a numeric text field with a bounded schema.
- The pasted holdings area is multiline with a full header-row example.
- Preset chips cover the default asset breakdown and Top-N sector grouping.

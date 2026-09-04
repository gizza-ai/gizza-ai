## About this tool

Use this calculator to turn pasted financial statement figures into a grouped ratio report. Paste the current period as `label: value` lines, using ordinary statement labels such as `Revenue`, `COGS`, `Current assets`, `Current liabilities`, `Total assets`, `Total liabilities`, `Total equity`, `Net income`, `Inventory`, `Accounts receivable`, `Accounts payable`, `Long term debt`, `Shares outstanding`, and `Share price`.

The parser tolerates spreadsheet-friendly amounts: currency symbols, thousands separators, accounting negatives like `(4,500)`, and scale suffixes such as `340k`, `1.2m`, or `2bn`. When enough parts are present it derives subtotals such as gross profit, operating income, total assets, total liabilities, total equity, and EBITDA, then says which figures were derived.

### Worked example

Paste this current-period statement:

```
Revenue: 1,200,000
COGS: 720,000
Operating expenses: 300,000
Depreciation and amortization: 40,000
Interest expense: 20,000
Taxes: 40,000
Net income: 120,000
Cash: 90,000
Accounts receivable: 150,000
Inventory: 180,000
Total current assets: 420,000
Fixed assets: 580,000
Accounts payable: 110,000
Short term debt: 60,000
Total current liabilities: 170,000
Long term debt: 330,000
Retained earnings: 200,000
Total equity: 500,000
```

With `groups=all`, `basis=average`, `days_in_period=365`, `benchmarks=true`, `decimals=2`, and `output=summary`, the report includes values such as current ratio `2.47x`, quick ratio `1.41x`, debt to equity `1.00x`, gross margin `40.00%`, net margin `10.00%`, ROA `12.00%`, ROE `24.00%`, turnover ratios, Altman Z-Score, a DuPont ROE breakdown, derived-line-item notes, and generic benchmark flags.

### Limits and edge cases

- `figures` is required and accepts up to **400** non-blank lines. Extra notes are ignored with a warning when they do not contain a recognized label and amount.
- `prior_figures` is optional. Supplying it adds prior and change columns; `basis=average` uses average current/prior balance-sheet values for returns and turnover ratios.
- `groups` can be `all`, `liquidity`, `leverage`, `margins`, `returns`, `efficiency`, or `market`. Market ratios require share count and share price.
- `days_in_period` must be **1 to 366**. Use 365 for a year, 360 for banker-style years, 90 for a quarter, or 30 for a month.
- Missing inputs show `n/a` with the exact fields needed instead of silently printing zero.
- Benchmark flags are generic rules of thumb only. This tool is educational arithmetic, not financial, investment, tax, or accounting advice.

## FAQ

<details>
<summary>Which ratios does it calculate?</summary>

It covers current, quick and cash ratios, net working capital, debt to equity, debt ratio, equity ratio, net debt, interest coverage, Altman Z-Score, gross/operating/EBITDA/pretax/net margins, ROA, ROE, ROCE, ROIC, asset and working-capital turnover, inventory/receivables/payables turnover, DIO, DSO, DPO, cash conversion cycle, DuPont ROE, EPS, P/E, earnings yield, book value per share, price to book, and market cap when the required inputs are present.

</details>

<details>
<summary>Do I need to fill every line item?</summary>

No. Paste the figures you have. Any ratio with enough inputs is computed, and ratios with missing inputs show `n/a` plus the missing fields. Common subtotals are derived when possible, for example `gross_profit = revenue - cogs` or `total_assets = current_assets + fixed_assets`.

</details>

<details>
<summary>How should I compare two periods?</summary>

Paste the latest statement in `figures` and the earlier statement in `prior_figures`. Leave `basis=average` when you want return and turnover denominators to use the average of current and prior balance-sheet values. Use `basis=ending` when you want only current-period balances.

</details>

<details>
<summary>Are the benchmark flags financial advice?</summary>

No. The benchmark flags and health score are generic educational rules of thumb, not industry-specific targets and not investment, tax, accounting, lending, or valuation advice. Always interpret ratios in context and verify material decisions with qualified professionals.

</details>

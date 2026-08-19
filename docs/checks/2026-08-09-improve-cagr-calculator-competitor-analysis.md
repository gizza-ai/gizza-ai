# cagr-calculator — competitor analysis (2026-08-09)

Scan done before finalizing the implementation. Notes are paraphrased observations of common CAGR calculators; no competitor wording, branding, or trademarks were reused.

## Scope check

Existing finance and statistics blocks were checked for overlap. `compound-interest-calculator` projects one starting principal with contributions/rates, `returns-risk-analyzer` works from return series, `moving-average` and other time-series tools compute transforms, and charting blocks visualize columns. None owns the simple workflow of pasting a value series and returning CAGR plus period-over-period growth rows. This is a buildable pure arithmetic transform.

## Competitors reviewed

1. **cagrcalculator.net — CAGR Calculator.** Focuses on the classic three-field form: initial value, final value, and number of years. Output is the compound annual growth rate, with formula explanation. Table-stakes: start/end values, elapsed years, percentage output, and an explanation of the formula.

2. **Omni Calculator — CAGR calculator.** Presents beginning value, ending value, elapsed years, and related investment growth fields. It emphasizes CAGR as a smoothed annual return and offers worked examples. Table-stakes: explicit year count, handling of future/projected values, percent formatting, and a clear warning that CAGR smooths volatility.

3. **CalculatorSoup — CAGR calculator.** Provides start value, end value, and periods/years, and shows formula/result. It also documents that CAGR is useful for business or investment performance over time. Table-stakes: simple numeric input, formula transparency, validation of positive values, and rounded percent output.

4. **Brokerage/bank CAGR calculators.** Several finance-site calculators expose only lump-sum initial/final investment and tenure, with investment-disclaimer copy. Table-stakes: positive monetary values, tenure in years, CAGR percent, and clear financial-context limitations.

## Table-stakes extracted → decisions

| Table stake | Decision | Where |
| --- | --- | --- |
| Start value, end value, years | in-model | `values` plus `years`; two pasted values cover the classic form |
| Whole series support | in-model addition | `values` accepts one number per line or `label,value` rows |
| Period spacing (annual/quarterly/monthly/etc.) | in-model | `period` enum |
| Exact date range | in-model | `start_date`, `end_date` |
| CAGR percent | in-model | summary/json output |
| Total growth, multiple, absolute change | in-model | summary/json output |
| Period-over-period growth table | in-model addition | summary/table/csv/json outputs |
| Best/worst periods | in-model addition | summary/json output |
| Doubling time and target projection | in-model | `target_value` and summary/json output |
| Decimal places | in-model | `decimals` integer 0..10 |
| Formula clarity and validation | in-model | page copy and error messages |
| Charts/interactive investment UI | out-of-model for this tool | belongs to charting/investment simulators, not a pure text transform |
| Brokerage advice or recommendations | out-of-model | page states arithmetic/educational only |

## Defaults chosen

- `period = annual`, matching the most common CAGR use case.
- `years = 0` means derive elapsed years from row count and period spacing.
- `output = summary` because users usually want headline metrics plus the period table.
- `decimals = 2`, matching common finance calculators.
- `has_header = false`, but a checkbox supports pasted spreadsheet columns.

## Examples to ship

- Two values over five years: demonstrates the classic CAGR form.
- Yearly labeled revenue series: demonstrates period-over-period growth.
- Monthly users with exact date range and CSV output: demonstrates header skipping, date override, and spreadsheet export.

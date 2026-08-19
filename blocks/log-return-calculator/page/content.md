## About this tool

A log return — also called a continuously compounded return, a logarithmic return, or a cc return — is `ln(price ÷ previous price)`. It is the return convention most quantitative work is built on, because two properties that simple percentage returns lack come for free.

The first is that log returns are **time-additive**. Add up the daily log returns of a month and you get exactly the month's log return, so there is no chain-linking, no multiplying `(1 + r)` factors, and no drift when you slice a series into sub-periods. The second is **symmetry**: a move up and the move that exactly undoes it are `+x` and `−x`. A price that goes 50 → 70 → 50 gives simple returns of `+40.00%` then `−28.57%`, which look lopsided, while the log returns are `+33.6472%` and `−33.6472%` and sum to zero.

Paste a column of prices or index levels — closes, NAVs, exchange rates, subscriber counts, anything that only ever goes above zero — and this page computes every step. Rows may be bare numbers, or `label,price` pairs like `2024-01-02,187.15` so the output keeps your dates. Currency symbols (`$`, `€`, `£`, `¥`) and thousands separators are stripped for you, and a single comma-separated line such as `100, 105, 103` is treated as a series too. Everything runs in WebAssembly in your browser; no prices are uploaded anywhere.

Alongside the per-period log return you get the simple return of the same step for comparison, a running cumulative log return, the total for the whole span, the mean per period, the sample standard deviation (the per-period volatility), and both figures scaled to a year using the periods-per-year you pick. The annualized simple figure is `exp(annualized log return) − 1`, which is the CAGR implied by the series.

### Worked example

Four monthly fund NAVs, annualized at 12 periods per year, percent units, 4 decimals:

```text
100
105
103
108
```

Output:

```text
Log returns for 4 prices → 3 returns (monthly, 12 periods/year)

first price: 100 at 1
last price:  108 at 4

total log return:        +7.6961%
same span, simple:       +8.0000%
mean log return:         +2.5654% per period
volatility (std dev):    3.8878% per period
annualized log return:   +30.7844% per year
annualized, simple:      +36.0489% per year
annualized volatility:   13.4677% per year
best period:             +4.8790% at 2
worst period:            -1.9231% at 3
positive periods:        2 of 3 (1 negative)

#  label  price  log return  simple return  cumulative log
----------------------------------------------------------
1  1        100           —              —               —
2  2        105    +4.8790%       +5.0000%        +4.8790%
3  3        103    -1.9231%       -1.9048%        +2.9559%
4  4        108    +4.7402%       +4.8544%        +7.6961%
```

Read the additivity straight off the table: `+4.8790 − 1.9231 + 4.7402 = +7.6961`, the total log return. The equivalent simple return over the same span is `+8.0000%`, because `exp(0.076961) − 1 = 0.08`.

Switch the output shape to **CSV** to paste the same table back into a spreadsheet as plain numeric columns, to **JSON** for every computed field including the per-step array, or to **table** for just the grid.

## Options and limits

- **Prices, not returns.** The input is a level series. Feeding it a column of percentages produces the log return *of the percentages*, which is meaningless.
- **Every price must be greater than zero.** `ln(0)` and `ln(negative)` are undefined, so a zero or negative level is rejected by row number rather than silently becoming `NaN` or `-inf`. That also means this tool cannot handle P&L series that cross zero.
- **At least 2 prices, at most 2,000.** `n` prices produce `n − 1` returns; the first row therefore shows an em dash instead of a return, because there is no earlier price to divide by.
- **Volatility needs at least 2 returns** (3 prices). With a single return the sample standard deviation is undefined and both volatility lines read `n/a`.
- **Decimal places run from 0 to 10.** Internally every return is rounded to 8 decimal places before display, so asking for 9 or 10 shows trailing zeros rather than extra precision.
- **Periods per year is a label, not a check.** Choosing 252 for a monthly series will happily annualize it as if it were daily. Pick the frequency your rows actually are.
- **Row spacing is ignored.** The tool counts rows, not calendar gaps, so a series with a missing week is treated as evenly spaced. Fill or drop gaps before pasting if that matters.
- **Parsing rules.** One point per line; `2024-01-02,AAPL,187.15` takes the last field as the price and the first as the label; a comma is read as a thousands separator only when the digit groups are valid (`1,234.50`), so a 4-digit lead group keeps the comma as a delimiter. Tick **First line is a header row** for exports that start with `date,close`.
- **Annualization is the square-root-of-time rule.** Volatility is scaled by `√periods_per_year`, which assumes returns are independent across periods. Real series with momentum or mean reversion break that assumption.

Educational only — not financial advice.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>When should I use log returns instead of simple percentage returns?</summary>

Use log returns whenever you are going to **add, average, or model** returns: summing sub-periods, computing a mean and standard deviation, fitting a distribution, or running a Monte Carlo. Use simple returns when you are reporting what actually happened to money over one span, or when you are weighting the returns of several holdings into a portfolio — simple returns are additive across assets, log returns are not. This page shows both columns side by side so you never have to pick blindly.

</details>

<details>
<summary>Why is the total log return smaller than the simple return?</summary>

Because `ln(1 + r)` is always less than `r` for positive `r`, and the gap widens as the move gets bigger. In the worked example above, `+8.0000%` simple is `+7.6961%` in log terms. They are the same underlying move written in two conventions, and `exp(log return) − 1` converts back exactly. For small moves the two are nearly identical, which is why day-to-day log returns look so much like percentages.

</details>

<details>
<summary>What does "periods per year" actually change?</summary>

Only the two annualized lines and the frequency label in the header. The mean log return is multiplied by the number you pick, and the volatility is multiplied by its square root. Nothing per-period changes. Pick 252 for daily rows from a market that trades on business days, 365 for calendar-daily data such as crypto or exchange rates, and 52, 26, 12, 4, or 1 for weekly, biweekly, monthly, quarterly, or annual rows.

</details>

<details>
<summary>My data has a zero or a negative value and the tool refuses it. What now?</summary>

That is intentional: `ln(price ÷ previous price)` is undefined unless both prices are strictly positive, so a zero would give `-inf` and a negative would give `NaN`. The error names the offending row so you can find it. If the zero is a missing quote from a holiday or a data outage, delete that row or carry the previous price forward. If your series genuinely crosses zero — a P&L curve, a net position, a spread — log returns are the wrong tool for it; work with differences instead.

</details>

<details>
<summary>Can I paste a whole spreadsheet column with dates, headers and currency symbols?</summary>

Yes. Copy the two columns straight out of the sheet: tab, comma, and semicolon are all accepted between the label and the price, `$`, `€`, `£`, `¥`, `₹`, `₽`, quotes, and thousands separators are stripped, and blank lines are dropped. If the first row is a header like `date,close`, tick **First line is a header row**. Rows without a label are numbered `1`, `2`, `3`, … in the output instead.

</details>

<details>
<summary>How is the volatility computed, and why does mine differ from another tool's?</summary>

It is the **sample** standard deviation (dividing by `n − 1`) of the per-period log returns, then multiplied by `√periods_per_year` to annualize. Two common sources of disagreement: some tools use the population formula (`n`), which reads slightly lower on short series, and some compute the standard deviation of simple returns rather than log returns. There is also no mean-subtraction variant here — this is the plain sample standard deviation about the sample mean.

</details>

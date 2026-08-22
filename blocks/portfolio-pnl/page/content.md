## About this tool

Paste positions from a spreadsheet or broker export and calculate portfolio profit and loss locally. Each row is `name, quantity, entry price, current price` with optional flat fees, dividends, days held, and a `long` or `short` side. The report shows cost basis, market value, net P/L, return percentage, fee-adjusted break-even prices, winners/losers, and optional tax estimates.

Worked example:

```text
AAPL, 50, 150, 187.50, 9.99
TSLA, 10, 250, 220
```

With a 15% tax rate, the report nets the winning and losing positions together before estimating tax. A negative quantity or a trailing `short` marks a short position; otherwise rows use the selected default side.

Limits and edge cases: the tool does not fetch live market prices, value currencies, or provide investment advice. It reads the prices you supply, accepts up to 5,000 positions, rejects negative prices, treats flat fees as costs, and uses the optional tax rate only on a positive portfolio-level net gain.

## FAQ

<details>
<summary>What columns can I paste?</summary>

Use `name, quantity, entry price, current price` as the first four columns. Optional trailing columns are flat fees, dividends or income collected, days held, and a `long` or `short` side word. Tab-separated rows work too, which is useful when prices contain thousands separators.

</details>

<details>
<summary>Does it download current prices?</summary>

No. This is an offline calculator: every current price comes from the text you paste. That keeps results deterministic and avoids sending portfolio data to a price API.

</details>

<details>
<summary>How are fees and taxes handled?</summary>

A row-level flat fee is subtracted from that position. The percent fee setting is applied to both the entry notional and the current notional for every row. The tax estimate is applied only after gains and losses are netted at the portfolio level, and only when the net result is positive.

</details>

<details>
<summary>Can I calculate short positions?</summary>

Yes. Add `short` to the end of a row, choose short as the default side, or enter a negative quantity. Short rows gain when the current price is lower than the entry price, and their break-even price includes fees just like long rows.

</details>

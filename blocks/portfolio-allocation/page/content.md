## What this tool does

Portfolio allocation is the percentage split of your holdings: how much sits in stocks, bonds, cash, one sector, one account, or one oversized position. Paste rows from a broker export or spreadsheet and this tool groups the values, calculates each slice's percent of the total, and returns a chart-ready table with proportional bars.

The conversion runs locally in WebAssembly. Nothing is uploaded, and the output is plain text you can paste into a report, spreadsheet, or charting tool.

## Input format

Use CSV or tab-separated rows. A header row is allowed and skipped automatically:

```text
Name, Value, Asset, Sector, Account
AAPL, 6000, Stocks, Technology, Brokerage
BND, 3000, Bonds, Bonds, IRA
Cash, 1000, Cash, Cash, Savings
```

Columns after value are optional. If you group by a missing field, the holding lands in `(unspecified)`.

Values can be plain numbers, currency amounts, accounting negatives, or a quick shares-at-price expression:

- `6000`
- `$1,234.50` (tab-separated rows work best when using thousands commas)
- `(500)` for a negative value
- `10 @ 150` or `10 x 150` for shares times price

## Options

| Option | Choices | What it does |
| --- | --- | --- |
| **Group by** | `asset` (default), `holding`, `sector`, `account` | Chooses the allocation dimension. |
| **Sort** | `value` (default), `label` | Sorts largest-first or alphabetically. Top-N is applied before label sorting. |
| **Top N slices** | `0` to `1000` | Use `0` to show everything, or keep the biggest N slices and fold the rest into `Other`. |
| **Currency prefix** | `$`, `€`, `£`, `USD`, blank | Prefix shown before values in the output table. |

## Example

With this input:

```text
Name, Value, Asset, Sector, Account
AAPL, 6000, Stocks, Technology, Brokerage
BND, 3000, Bonds, Bonds, IRA
Cash, 1000, Cash, Cash, Savings
```

Grouped by **asset**, the output starts:

```text
Allocation by asset class — $10,000.00 total across 3 holdings

Stocks  $6,000.00   60.00%  ██████████████··········  (1 holding)
Bonds   $3,000.00   30.00%  ███████·················  (1 holding)
Cash    $1,000.00   10.00%  ██······················  (1 holding)
```

## FAQ

<details>
<summary>Is this financial advice?</summary>

No. It only calculates percentages from numbers you provide. It does not recommend trades, target allocations, or investments.

</details>

<details>
<summary>Can I paste a brokerage CSV export?</summary>

Yes, if you reduce it to the columns this tool expects: name, value, and optional asset, sector, and account. A spreadsheet copy-paste with tabs works well, especially for values that contain comma thousands separators.

</details>

<details>
<summary>What does Top N do?</summary>

Top N keeps the largest slices and folds the rest into an `Other` row. That is useful for charting a portfolio with many small positions while keeping the chart readable.

</details>

<details>
<summary>What is the concentration score?</summary>

The HHI score squares each slice's percentage and sums the result. Higher numbers mean more concentration. The tool labels the score as well diversified, moderately concentrated, or highly concentrated.

</details>

<details>
<summary>What are the limits?</summary>

The tool accepts up to 5 MB of pasted text and 100,000 holdings. It does not fetch live prices, identify tickers, infer sectors automatically, or connect to brokerage accounts.

</details>

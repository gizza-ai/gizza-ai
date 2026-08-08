## About this tool

Drawdown Analyzer measures how far a portfolio, strategy, index, or account balance fell from its own running high-water mark. Paste equity values, balance levels, prices, or periodic returns and the tool reports the maximum drawdown, peak and trough locations, drawdown duration, recovery time, current drawdown, average depth, longest underwater stretch, time underwater, ulcer index, pain index, and the deepest episodes.

Use plain values for an equity curve, or switch to Returns when the series is made of periodic returns such as `1.2%` or `-0.8%`. Rows can also be dated as `YYYY-MM-DD,value`, or you can provide a start date plus a frequency so the report labels peak, trough, and recovery dates. The underwater plot is fixed-width text so it is deterministic, copyable, and easy to compare in a report.

Worked example:

```text
10000
11200
9800
10400
12100
11600
13000
```

This series reaches a high at 11200, falls to 9800, recovers above the old high at 12100, and later makes a smaller drawdown from 12100 to 11600 before recovering at 13000. The output ranks those episodes by depth and also shows the current drawdown at the final observation.

Limits and conventions: the series must contain 2 to 20000 observations. Equity levels must be positive; returns must be greater than -100%. Drawdown is measured against the series' own running peak, not a rolling window. An episode is considered recovered only when the series closes back at or above the prior peak. Trading-day dates skip weekends but do not include exchange-specific holidays. Results are educational calculations, not financial advice.

## FAQ

<details>
<summary>Should I choose equity or returns?</summary>

Choose **equity** for account balances, prices, index levels, or already-compounded equity curves such as `10000, 10400, 9800`. Choose **returns** when each row is a periodic return such as `1.2%`, `-0.8%`, or decimal returns like `0.012`. Returns are compounded into a synthetic equity curve before drawdown is measured.

</details>

<details>
<summary>How are drawdown duration and recovery time counted?</summary>

The decline period counts from the peak observation to the trough. Recovery time counts from the trough back to the first observation at or above the old peak. The underwater stretch counts the whole episode from peak to recovery, or from peak to the final observation if the drawdown is still ongoing.

</details>

<details>
<summary>Can I paste dates?</summary>

Yes. Paste `YYYY-MM-DD,value` rows, one row per observation, or provide a start date and frequency. Dated rows take precedence over the start date field. Dates must run oldest first; the trading frequency advances over weekdays only and does not know market holidays.

</details>

<details>
<summary>What do ulcer index and pain index mean here?</summary>

The ulcer index is the root-mean-square of the underwater curve, so deeper and longer drawdowns weigh more heavily. The pain index is the mean absolute drawdown across all observations. Both include observations at new highs as zero drawdown.

</details>

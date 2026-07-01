## About this tool

The RSI calculator computes J. Welles Wilder's Relative Strength Index from a pasted series of prices, ordered oldest to newest. It accepts values separated by spaces, commas, semicolons, tabs, or newlines, so you can paste a column from a spreadsheet or a short CSV row directly.

The output JSON includes:

- the RSI value at each input point (`null` during the warm-up period),
- Wilder-smoothed average gains and losses,
- the latest RSI value,
- a latest signal (`overbought`, `oversold`, or `neutral`) based on your thresholds.

By default the tool uses the standard 14-period RSI with 70/30 overbought/oversold thresholds. Change the period or thresholds to match your strategy or charting package.

All computation happens in your browser; your price series is not uploaded.

## FAQ

<details>
<summary>How many prices do I need for a 14-period RSI?</summary>

At least `period + 1` — 15 prices for the default 14-period RSI — because the
first RSI value needs 14 price *changes*. Points before that are reported as
`null` (the warm-up), and the series is capped at 100,000 data points with a
maximum period of 10,000.

</details>

<details>
<summary>Why doesn't the RSI here match my charting platform exactly?</summary>

This tool uses Wilder's original method: the averages are seeded with the
simple mean of the first `period` gains/losses, then advanced with Wilder's
smoothing `avg = (avg_prev · (period − 1) + current) / period`. Platforms
that seed differently (or use an EMA-based variant) converge to the same
values but can differ slightly on the earliest readings — feed in more
history and the numbers align.

</details>

<details>
<summary>Does the order I paste the prices in matter?</summary>

Yes — the series must be **oldest to newest**. If you paste a column that's
sorted newest-first (as many data exports are), the gains and losses invert
and the RSI comes out mirrored. Values can be separated by spaces, commas,
semicolons, tabs, or newlines.

</details>

<details>
<summary>What do the overbought/oversold thresholds change?</summary>

Only the `signal` classification of the latest reading. The defaults are the
classic 70/30; both must lie between 0 and 100 with overbought above
oversold. The RSI values themselves are unaffected — RSI is 100 when there
are no losses in the window and 0 when there are no gains.

</details>

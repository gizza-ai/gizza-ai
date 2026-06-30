## About this tool

The RSI calculator computes J. Welles Wilder's Relative Strength Index from a pasted series of prices, ordered oldest to newest. It accepts values separated by spaces, commas, semicolons, tabs, or newlines, so you can paste a column from a spreadsheet or a short CSV row directly.

The output JSON includes:

- the RSI value at each input point (`null` during the warm-up period),
- Wilder-smoothed average gains and losses,
- the latest RSI value,
- a latest signal (`overbought`, `oversold`, or `neutral`) based on your thresholds.

By default the tool uses the standard 14-period RSI with 70/30 overbought/oversold thresholds. Change the period or thresholds to match your strategy or charting package.

All computation happens in your browser; your price series is not uploaded.

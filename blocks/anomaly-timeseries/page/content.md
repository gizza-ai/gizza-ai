## About this tool

Use this anomaly detector when you have a metric history — daily sign-ups, hourly error counts, weekly revenue — and you want to know which points genuinely broke from their own recent behaviour. A single mean and standard deviation over the whole series is the wrong yardstick for data that trends or repeats: it calls every Saturday an anomaly and misses a slow drift entirely. This tool scores each point against a **moving baseline** instead, and can judge it against **the same position in earlier cycles** when the metric has a weekly or monthly shape.

Paste the values in time order: one number per line, `label,value` rows so dates keep their names, or a single line separated by commas, spaces, semicolons or tabs. A leading header row is skipped.

Worked example: paste `120, 132, 118, 127, 131, 124, 129, 122, 135, 128, 126, 133, 410, 130, 125` with `method = rolling_z`, `window = 7`, `min_periods = 3` and `decimals = 2`. Row 13 comes back `critical`: its value is `410` against an expected `128.14` — the mean of the seven days before it — a normal band of `114.13 … 142.15`, and a score above `60`. Rows 1 and 2 come back `unscored`, because two neighbours are not enough history to judge anything against.

Which rule to use:

- `rolling_z` (default) — mean ± standard deviation of the `window` points around each point. The everyday choice.
- `rolling_mad` — median ± scaled median absolute deviation. Pick this when the window itself contains spikes: one big outlier inflates a standard deviation enough to hide the *next* outlier, and a median does not care.
- `seasonal_z` — compare each point with the points at the same position in the other cycles (same weekday at `period = 7`, same month at `period = 12`). This is what stops a routinely quiet Sunday being reported as an anomaly every week.
- `combined` — take the worse of the rolling and seasonal verdicts and report which rule fired, in the `rule` column.

How the verdict is decided: every baseline **excludes the point being tested**, so a spike can never hide inside the average it is compared against. `threshold` (default 3) is the line that flags an anomaly; `warn_threshold` (default 2) only labels near-misses `warning`, so a watch band never inflates the anomaly count. `direction` narrows alerts to spikes or to drops. `tolerance` is a deadband in the series' own units — set it when a metric is so flat that a rounding-level wobble scores like a crisis.

Limits and edge cases: 3 to 20,000 values; `window` and `period` go up to 1000. With `center = false` (the default, matching live monitoring) only earlier points feed a baseline, so the first rows are `unscored` — switch `center` on for a retrospective pass that scores the whole series, including its start. The seasonal rules need at least three points at each position in the cycle, which is `2 × period + 1` values. If a baseline is perfectly flat, no finite score exists: the point is reported with a `null` score and flagged only when it clears the tolerance, and `summary.flat_baseline` counts those rows.

## FAQ

<details>
<summary>What does the score mean?</summary>

It is the distance from the expected value measured in baseline spreads — a rolling z-score for `rolling_z` and `seasonal_z`, and a scaled median-absolute-deviation score for `rolling_mad`. A score of `3` means the point sits three spreads from where its neighbours said it should be. `3` is the usual production cutoff; `2` is roughly the most extreme 5% of normally-distributed points, which is why it is the default watch band rather than the alert line.

</details>

<details>
<summary>Why are the first rows "unscored" instead of "normal"?</summary>

Because they have no history yet. With the default `center = false`, a point is judged only against points that came before it, so the first `window` rows have fewer peers than required and get no verdict at all — an honest blank rather than a verdict based on two numbers. Lower `min_periods` to start scoring sooner, or switch on the centred window to use later points as well.

</details>

<details>
<summary>My weekends are always low and every one is flagged. What should I change?</summary>

Switch `method` to `seasonal_z` and set `period` to the cycle length (7 for daily data with a weekly rhythm, 24 for hourly, 12 for monthly). Each point is then compared with the other points at the same position in the cycle, so a quiet Saturday is measured against other Saturdays. Use `combined` if you also want the rolling rule's verdict, which catches breaks that keep the weekly shape but shift the whole level.

</details>

<details>
<summary>How is this different from a plain outlier check on a list of numbers?</summary>

A plain check compares every value with one global mean, standard deviation or quartile range, and it ignores the order of the data. That is the right tool for an unordered sample. A time series is ordered, usually trending and often seasonal: here the yardstick moves with the data, so a point is only anomalous relative to *its own* neighbourhood or *its own* phase in the cycle.

</details>

<details>
<summary>What do the lower and upper columns show?</summary>

The normal band: `expected ± threshold × spread`, the range the point would have had to stay inside to pass. It is handy for charting an envelope around the series, and for explaining a verdict to someone who does not want to read scores. The band is blank when the baseline was perfectly flat, because there is no spread to scale.

</details>

<details>
<summary>Does my data leave the browser?</summary>

No. The detector is compiled to WebAssembly and runs on this page, so the series you paste is never uploaded anywhere. The same tool is available from the command line for scripted runs.

</details>

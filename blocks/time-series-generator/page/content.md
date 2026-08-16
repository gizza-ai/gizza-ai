## About this tool

Time series generator creates reproducible test data for charts, forecasting pipelines, monitoring demos and data-cleaning fixtures. Pick a start timestamp, interval and row count, then layer on a trend, one or more seasonal cycles, seeded noise, missing values and outliers.

Use it when you need data that looks realistic enough to exercise code paths, but you still want every run to be deterministic. The same seed and settings produce the same CSV, TSV, JSON or NDJSON in the CLI, chat block and browser page.

Example: generate two days of hourly web traffic with daily seasonality and two value columns:

```bash
gizza tool time-series-generator start=2024-06-01T00:00:00Z interval=1h count=48 base=1000 period=24 amplitude=120 noise_level=20 series=2 labels=visits,signups seed=7
```

For QA fixtures, switch `output=stats` to inspect the achieved min, max, mean, standard deviation and how many values were blanked or spiked before handing the rows to another tool.

## Limits and edge cases

- Up to 100,000 rows, 20 series columns and 200,000 total emitted values.
- `period` and `amplitude` accept comma-separated lists, so `period=24,168 amplitude=8,4` combines daily and weekly cycles.
- `interval` supports fixed units (`ms`, `s`, `m`, `h`, `d`, `w`) and calendar units (`mo`, `q`, `y`).
- `missing_rate`, `outlier_rate` and all noise streams are seeded and deterministic.
- CSV/TSV missing values are empty cells; JSON/NDJSON missing values are `null`.
- This tool emits data only. It does not draw a chart; paste the CSV into the charting tool of your choice.

## FAQ

<details>
<summary>How do I make the same data again?</summary>

Keep the same `seed` and the same settings. Randomness comes from a deterministic SplitMix64 stream, so matching inputs reproduce exactly across the CLI and browser page.

</details>

<details>
<summary>Can I generate daily and weekly seasonality at the same time?</summary>

Yes. Put comma-separated cycles in `period` and matching strengths in `amplitude`, for example `period=24,168 amplitude=8,4` for hourly data with a daily and weekly wave.

</details>

<details>
<summary>What does multiplicative mode change?</summary>

In additive mode, seasonality and noise are added in raw units. In multiplicative mode, `amplitude` and `noise_level` are fractions of the current level, which is useful when larger values should also have larger swings.

</details>

<details>
<summary>How are missing values and outliers represented?</summary>

Missing values become empty cells in CSV/TSV and `null` in JSON/NDJSON. Outliers are applied before min/max clamps, so you can create spikes and still keep the final data inside a realistic range.

</details>

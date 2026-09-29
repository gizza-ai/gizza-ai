## About this tool

Isolation forests flag rows that are easy to separate from the rest of a table. Each tree repeatedly picks a random split, and unusual rows tend to land alone after fewer splits, giving them a shorter average path length and a higher anomaly score. This tool runs the model locally in Rust/WebAssembly over pasted CSV, TSV, semicolon, pipe, or whitespace-delimited data.

Use it for quick checks on sensor readings, billing rows, feature tables, operational metrics, risk scores, and other numeric tabular data where you need a ranked shortlist of suspicious rows rather than a trained supervised classifier. The output includes the score, average path length, anomaly flag, rank, and enough settings to reproduce the run exactly.

## Worked example

Paste a small sensor table where one reading is far away from the normal cluster:

```text
temp,pressure
20,101
21,102
20,100
22,101
21,101
20,102
21,100
90,300
```

With the default forest and **Sort rows = Most anomalous first**, the outlying row rises to the top:

```text
Isolation Forest — standard (axis-parallel splits)

Rows scored:      8 of 8 data rows
Features:         2 of 2 columns — temp, pressure
Trees:            100
Sample size:      8 (auto)
Score threshold:  0.5000 — auto (0.5)
Anomalies:        1 of 8 rows (12.50%)

Rank Row  Score   Path Flag    temp pressure
   1   8 0.7815 1.2200 ANOMALY   90      300
```

The exact score can move when you change the seed, tree count, sample size, or method, but the same input plus the same seed is deterministic.

## Choosing the controls

- **Method**: `standard` uses the original axis-parallel splits and is scale-invariant per column. `extended` uses random hyperplanes and standardizes features internally; it is often better for diagonal or correlated anomalies.
- **Trees** controls stability. More trees smooth the average path length; fewer trees run faster and are useful for exploratory runs.
- **Sample size** defaults to `auto` = `min(256, rows)`, the standard isolation-forest setting. You can also enter a row count, a fraction like `0.25`, or a percentage like `25%`.
- **Max features** samples a subset of columns per tree. `1` means all selected features, `0.5` means half of them, and values above 1 are treated as a column count.
- **Contamination** sets how many rows to flag: `auto` uses the classic 0.5 score cut-off, while `0.05` or `5%` flags the top 5% by score. **Threshold** overrides this when you know the score cut-off you want.
- **Missing cells** can drop rows, fill from the median or mean, fill zero, or fail fast with an error naming the cell.
- **Sort rows**, **Top N**, and **Only anomalies** only change what gets listed; the model is always fit over every usable row.

## Limits and edge cases

- Up to **20,000 data rows**, **200 columns**, and **1,000 trees** per run.
- At least two usable rows and at least one numeric feature column are required.
- Blank, `NA`, `N/A`, `NaN`, `null`, `none`, `-`, `?`, and `.` are treated as missing values.
- Non-numeric columns are ignored when **Feature columns** is blank. If you explicitly name a non-numeric feature, use a missing policy that can handle it or the run will explain the bad cell.
- This is unsupervised anomaly scoring, not a diagnosis. A flagged row is unusual relative to the pasted table; domain review decides whether it is an error, fraud, seasonality, a new population, or valid rare behaviour.
- Isolation forests work best on numeric features with meaningful distances. IDs, timestamps encoded as arbitrary integers, or one-hot categories can dominate the score if included blindly.

## FAQ

<details>
<summary>Is a higher score always more anomalous?</summary>

Yes. Scores are in the isolation-forest convention where higher means easier to isolate and therefore more anomalous. Values around 0.5 are the classic boundary used by `contamination = auto`; values well above 0.5 are isolated quickly, and lower values look like the bulk of the data.

</details>

<details>
<summary>Should I choose standard or extended?</summary>

Use `standard` first. It is the original algorithm, fast, and invariant to rescaling individual columns. Use `extended` when the normal data forms diagonal bands or correlated clouds: hyperplane cuts can isolate points outside that shape without drawing axis-aligned rectangles around it. Extended mode standardizes features internally because hyperplanes mix columns.

</details>

<details>
<summary>How should I set contamination?</summary>

If you do not know the expected outlier rate, keep `auto` and review the top scores. If you have a policy like "investigate the top 2% of rows", set `contamination = 2%` and sort by score. This changes the flag threshold, not the raw anomaly scores.

</details>

<details>
<summary>Why did some rows get skipped?</summary>

With the default `missing = drop`, any row with a blank or non-numeric value in a selected feature column is skipped for scoring and listed without a score. Switch to `median`, `mean`, or `zero` to fill missing cells, or `error` if you want the run to stop and name the first bad cell.

</details>

<details>
<summary>Can I compare scores across different tables?</summary>

Only cautiously. Scores are relative to the forest built from the current table, its features, sample size, seed, and contamination policy. Use them to rank rows within one run. For monitoring over time, keep the same feature set and settings and compare ranks or flagged rates alongside the raw scores.

</details>

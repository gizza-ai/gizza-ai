## About this tool

Data drift is the quiet failure mode behind many broken dashboards, machine-learning models and data-quality checks: the schema still loads, but the population changed. A numeric column shifts upward, a nullable field starts arriving blank, a category appears that the training data never saw, or a renamed column silently disappears from the current export.

This tool compares a **reference** dataset against a **current** dataset and summarizes the movement column by column. Paste CSV, TSV, semicolon-separated or pipe-delimited text, choose whether the first row is a header, and get a report that shows the inferred type on each side, null-rate change, distinct-count change, numeric range and mean, new or missing category values, schema additions/removals, and a drift score.

The default metric is **Population Stability Index (PSI)** because it is simple, deterministic and common in monitoring reports. You can switch to **Jensen-Shannon distance** when you want a bounded 0–1 score. Numeric columns with more than the configured `max_categories` distinct values are binned using reference-derived bins; lower-cardinality columns are compared as categories so new values are easy to spot.

### Worked example

Reference dataset:

```csv
id,amount,country
1,10,US
2,12,US
3,11,FR
4,13,FR
```

Current dataset:

```csv
id,amount,country
1,40,US
2,44,US
3,,FR
4,47,DE
```

With the defaults and `columns = amount,country`, the report flags dataset drift. `amount` moves from the 10–13 range to 40–47 and gains a 25% null rate; `country` gains the new category `DE`. That gives you a reviewable summary for a pull request, incident note or CI log without opening a notebook.

### Limits and edge cases

- The tool is for pasted tabular text, not database connections or hosted monitoring.
- Each side is capped at 100,000 rows to keep browser and CLI runs predictable.
- Blank cells and common tokens such as `NA`, `N/A`, `NULL`, `NaN`, `None`, `nil` and `-` count as nulls.
- Header mode matches columns by name, so reordered columns still line up. Headerless mode compares by position as `col1`, `col2`, and so on.
- PSI and Jensen-Shannon are distribution distances, not formal p-values. They are stable and reproducible, but they do not replace a statistical test tailored to a specific experiment.
- Interactive histograms, model-performance drift and multivariate/domain-classifier drift are out of scope for this pure Rust text report.

## FAQ

<details>
<summary>What is the difference between PSI and Jensen-Shannon distance?</summary>

PSI is an unbounded monitoring score that is widely used in credit-risk and ML operations reports. Roughly, 0.1 is a moderate shift and 0.2 is often treated as significant. Jensen-Shannon distance is bounded between 0 and 1, so it is easier to compare across columns; 0 means identical distributions. Both are computed from the same per-column bins or category counts in this tool.

</details>

<details>
<summary>Why are some numeric columns reported as categorical?</summary>

A numeric-looking column with only a few distinct values often behaves like a category: status codes, rating buckets, cohort ids, boolean flags encoded as 0/1. The `max_categories` control decides that cutoff. At or below it, the report lists new and missing values; above it, the numeric values are binned and compared as a distribution.

</details>

<details>
<summary>How should I choose a threshold?</summary>

For PSI, start with 0.2 when you only want significant shifts and 0.1 when you want an earlier warning. For Jensen-Shannon distance, 0.1 is a common practical starting point. The best threshold depends on column importance and expected seasonality, so use the report to rank columns first and then tune the cutoff for your dataset.

</details>

<details>
<summary>Can this compare train and production data for an ML model?</summary>

Yes, if you export both sides as tabular text. Use the training or validation set as the reference and recent production examples as the current dataset. The report will catch schema changes, null-rate spikes, numeric shifts and new categories. It does not evaluate model predictions or labels; that requires a separate model-performance drift check.

</details>

<details>
<summary>What happens when columns are renamed or missing?</summary>

With headers enabled, columns that appear only in the reference are listed as removed and columns that appear only in the current data are listed as added. Those schema changes contribute to the dataset-level drift verdict, even though there is no per-column distribution score for a missing counterpart.

</details>

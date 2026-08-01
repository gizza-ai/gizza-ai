## About this tool

**Correlated Feature Pruner** reduces multicollinearity in a numeric dataset by
dropping redundant columns. It computes the pairwise correlation between your
columns (each column is a feature, each row an observation) and greedily removes
one column from every pair whose **absolute** correlation is stronger than the
threshold you set. The first-seen column of each correlated group is kept, so the
result is deterministic. Everything runs locally in your browser — nothing is
uploaded.

This is the same "keep the first, drop the later" recipe used by common
pandas / feature-engine multicollinearity filters, done in one paste. Highly
collinear features carry the same information, inflate variance in linear models,
and make coefficients unstable — pruning them before modeling keeps your feature
set lean and interpretable.

### Worked example

Input (with a header row):

```
age,income,savings
20,100,5
30,200,9
40,300,2
50,400,7
```

Here `income` moves in lockstep with `age` (|r| = 1.00), while `savings` is
essentially uncorrelated. With the default threshold `|r| > 0.90` and Pearson
correlation:

```
Kept 2 of 3 columns (threshold |r| > 0.90, pearson).

Kept (2): age, savings
Dropped (1):
  income (|r|=1.00 with age)

Pruned data:
age,savings
20,5
30,9
40,2
50,7
```

Each dropped column reports the correlation value and the kept column it was
redundant with, and the pruned dataset is emitted as CSV of the kept columns.

### Options

- **Threshold (|r|)** — the absolute-correlation cutoff, from 0 to 1 (default
  0.9). A column is dropped when its |correlation| with an already-kept column is
  strictly greater than this. Lower it (e.g. 0.8) to prune more aggressively;
  raise it (0.95) to keep more. The comparison is exclusive, so `|r| = 1.0` at
  threshold `1.0` keeps both.
- **Correlation method** — *Pearson* (linear, default), *Spearman* (rank-based,
  catches monotonic-but-nonlinear relationships), or *Kendall* (tie-corrected
  tau-b, robust rank concordance).
- **Column names** — optional comma-separated labels; otherwise columns default to
  `v1..vN` (or the header row).
- **First row is a header** — treat the first line as column names instead of
  data.

### Limits

- The pruning is greedy and order-dependent by design: within a correlated group
  the left-most column is kept. Reorder your columns (or rename via the labels
  field) if you want a different survivor.
- A constant column has undefined correlation; it is treated as uncorrelated, so
  it is never dropped and never forces a drop.
- Correlation measures pairwise redundancy, not a full multicollinearity
  diagnostic — three columns can be jointly collinear while no single pair trips
  the threshold. Use a VIF check for that case.
- Everything happens in-browser on a single paste; there is no file upload or
  server-side batch. Use **Copy** or **Download** for the result.

## FAQ

<details>
<summary>How does it decide which column of a correlated pair to drop?</summary>

It walks the columns left to right and keeps a column unless it is too correlated
with a column already kept — in which case that later column is dropped. So the
**first-seen** column of each correlated group survives. This makes the result
deterministic; reorder your columns if you want a different one to be kept.

</details>

<details>
<summary>What's the difference between Pearson, Spearman, and Kendall?</summary>

*Pearson* measures linear correlation on the raw values. *Spearman* correlates the
**ranks**, so it catches monotonic relationships even when they are nonlinear
(e.g. `y = x³`). *Kendall* is a rank concordance measure (tie-corrected tau-b),
often preferred for small samples or many ties. Switch methods if a linear
correlation misses a redundancy you can see.

</details>

<details>
<summary>Does a negative correlation count?</summary>

Yes. The threshold is applied to the **absolute** value, so a strong negative
correlation (e.g. `r = -0.98`) is just as redundant as a strong positive one and
will trigger a drop. The reported value keeps its sign so you can see the
direction.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely in your browser. Your
numbers never leave your device, so it is safe to paste private or unpublished
datasets.

</details>

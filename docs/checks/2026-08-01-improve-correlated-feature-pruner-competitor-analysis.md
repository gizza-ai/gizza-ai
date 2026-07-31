# correlated-feature-pruner — competitor analysis (2026-08-01)

Tool: compute pairwise correlations across numeric feature columns and drop one
column from each pair whose absolute correlation exceeds a threshold. Pure,
browser-local, no upload. Findings below are paraphrased from public docs and
search results; no competitor copy, branding, or trademarks are reused.

## Search

Queries: `correlated feature selection remove highly correlated features online
tool threshold pearson spearman` and `Drop correlated features threshold Pearson
Spearman Kendall feature selection tool documentation`.

The practical competitors are mostly Python/data-science library functions rather
than standalone paste-box web tools, but their parameter surface defines the
expected behavior for this utility.

## Competitor profiles

### 1. Feature-engine `DropCorrelatedFeatures`
- **Inputs:** pandas DataFrame; optional subset of variables to evaluate.
- **Options:** correlation `method` (anything supported by `pandas.corr`, including
  Pearson, Spearman, and Kendall), `threshold` (example/default commonly 0.8),
  missing-value handling, and variable confirmation.
- **Behavior:** computes absolute correlations, forms correlated feature groups,
  keeps the first feature encountered, and drops later correlated variables.
- **Outputs:** transformed data plus attributes listing correlated groups and
  features to drop.

### 2. Featuretools `remove_highly_correlated_features`
- **Inputs:** feature matrix / generated feature definitions.
- **Options:** `pct_corr_threshold` (documented default 0.95), `features_to_check`,
  and `features_to_keep`.
- **Behavior:** removes columns that are highly correlated with another column;
  order matters, with later/right-side generated features treated as more complex.
- **Outputs:** pruned feature matrix and, when provided, the matching pruned feature
  definitions.

### 3. Melampus feature selector `drop_correlated_features`
- **Inputs:** CSV or DataFrame.
- **Options:** user-provided correlation score and metric (`pearson`, `kendall`,
  `spearman`, or compatible callable).
- **Behavior:** builds an absolute correlation matrix, examines the upper triangle,
  and drops columns whose correlation with any earlier column exceeds the score.
- **Outputs:** a DataFrame with the high-correlation columns removed.

## Table-stakes params + decisions

| capability | seen at | decision | tag |
| --- | --- | --- | --- |
| Numeric table input | all | `data` multiline paste, comma/space/tab separated | in-model |
| Header row / column names | all | `header` checkbox plus optional `labels` CSV override | in-model |
| Absolute-correlation threshold | all | `threshold` number/slider, default `0.9`, bounded 0..1 | in-model |
| Pearson correlation | all | `method=pearson` default | in-model |
| Spearman correlation | Feature-engine, Melampus | `method=spearman` rank transform | in-model |
| Kendall correlation | Feature-engine, Melampus | `method=kendall` tau-b | in-model |
| Keep-first/drop-later deterministic rule | Feature-engine, Melampus; Featuretools is order-sensitive | implemented left-to-right with dropped-vs-keeper report | in-model |
| Output transformed/pruned data | all | append `Pruned data:` CSV of kept columns | in-model |
| Report dropped groups/features | Feature-engine | report kept list and per-drop reason/correlation | in-model |
| `features_to_keep` pinned columns | Featuretools | useful but secondary; out-of-model for first browser paste version | considered, deferred |
| Missing values / categorical handling | Feature-engine | current tool requires finite numeric values; missing/categorical cleaning belongs to other tools | out-of-model |
| DataFrame/feature definition object preservation | Featuretools | browser tool emits CSV text only | out-of-model |

## UX control patterns adopted

- Multiline textarea for numeric rows.
- Threshold slider with visible numeric bounds.
- Correlation method select (enum → manifest control labels).
- Header checkbox and labels text field.
- Preset chips for a header-row example, aggressive threshold, and Spearman
  monotonic example.
- Text output designed for exact comparison: summary, kept/dropped lists, and
  pruned CSV.

## Engine decision

Use a small pure-Rust implementation: parse numeric rows; compute Pearson;
rank-transform for Spearman; compute Kendall tau-b in O(n²); greedily keep columns
left-to-right and drop later columns whose absolute correlation is strictly above
the threshold. This fits the current gizza model and avoids any Python/pandas
runtime dependency.

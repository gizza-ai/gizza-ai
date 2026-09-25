# isolation-forest-anomaly — competitor analysis (2026-09-23)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are paraphrased
from public documentation; no competitor copy, branding or trademarks are reproduced here or in
the shipped page.

## Duplicate check (done first)

`ls blocks/ | grep -iE 'anomal|outlier|forest|isolat|cluster|stat'` plus a grep for
`isolation forest|anomaly` across `blocks/*/core/src/lib.rs`.

| Existing block | What it does | Overlap verdict |
| --- | --- | --- |
| `outlier-detector` | z-score / modified z-score / Tukey IQR fences over **one pasted list of numbers** | Univariate only. Cannot see a row that is unremarkable in every single column but impossible as a combination. No overlap. |
| `iqr-outlier-trimmer` | Tukey fences **per CSV column**, then removes/keeps/clips/flags rows | Still per-column thresholds; decides one column at a time. An isolation forest scores the whole row jointly and returns a continuous score, not a fence test. No overlap. |
| `data-clusterer` | KMeans / DBSCAN / hierarchical + PCA scatter plot | Groups rows; DBSCAN's noise label is a side effect of density clustering, not a ranked per-row anomaly score. Different question. |
| `descriptive-stats`, `csv-stats`, `z-score-calculator`, `z-score-normalize` | Summary statistics / standardization | No anomaly ranking at all. |
| `svm-classifier` | Supervised C-SVC, requires a labelled target column | Unsupervised vs supervised; the block explicitly errors when the target has one class. No one-class SVM arm exists. |
| `email-spam-score` | The only block matching `anomaly` — a heuristic spam rubric for email text | Unrelated. |

Not a duplicate: no block does multivariate, unsupervised, per-row anomaly scoring. Build proceeds.

## Competitors reviewed

1. **MetricGate — Isolation Forest calculator** (browser-based statistics site,
   `metricgate.com/docs/isolation-forest/`). Closest direct competitor: upload a CSV/Excel file or
   use a built-in sample, pick numeric feature columns, set number of trees (default 100),
   subsample size (default 256) and contamination (default 0.05), then get anomaly scores, path
   lengths, a flagged-observations table ranked by score, a score histogram, a two-feature scatter
   plot, and summary statistics (count and percent flagged, the score threshold, mean score and
   mean path length split by normal vs flagged).
2. **scikit-learn `IsolationForest`** (the de-facto reference API that every other implementation
   is compared against, including the calculators). Parameters: `n_estimators` (100),
   `max_samples` (`auto` → min(256, n), or an int, or a fraction), `contamination` (`auto` or a
   fraction — `auto` pins the decision offset at −0.5), `max_features` (1.0 — fraction or count of
   features drawn per estimator), `bootstrap` (False), `random_state`. Methods: `score_samples`
   (the raw normality score), `decision_function` (score minus the offset) and `predict`
   (+1 inlier / −1 outlier).
3. **Orange Data Mining — Outliers widget** (desktop visual-programming tool). Exposes isolation
   forest alongside local outlier factor and one-class SVM, with contamination as the single
   headline control, and emits the input table split into inliers and outliers.
4. **Extended Isolation Forest** (the published variant, arXiv:1811.02141, shipped by the `eif`
   package and H2O). Splits on randomly-oriented hyperplanes (a random normal vector plus a random
   intercept) instead of axis-parallel cuts, which removes the rectangular banding artifacts the
   original algorithm leaves in the score field for correlated data.

## Table-stakes → where each one landed

| Capability seen in competitors | Fit | Where it landed |
| --- | --- | --- |
| Number of trees, default 100 | in-model | `trees` (1–1000, slider) |
| Subsample size, default 256 / `auto` | in-model | `sample_size` — `auto`, an integer, or a fraction |
| Contamination, `auto` or a fraction | in-model | `contamination` — `auto`, `0.05`, or `5%` |
| Explicit score cut-off instead of a rate | in-model | `threshold` (0 = derive from contamination) |
| Feature-column selection | in-model | `features` by header name or 1-based index; blank = every fully numeric column |
| `max_features` per tree | in-model | `max_features` (fraction ≤ 1, or an integer count) |
| `bootstrap` sampling | in-model | `bootstrap` checkbox |
| Reproducible `random_state` | in-model | `seed` |
| Per-row anomaly score in 0–1, higher = more anomalous | in-model | `anomaly_score` column, the 2^(−E(h)/c(ψ)) normalization |
| Per-row average path length | in-model | `path_length` column; mean path length also in the summary |
| Ranked flagged-observations table | in-model | `sort = score` plus `top` and `only_anomalies` |
| Flag count, percentage and effective threshold | in-model | text/JSON summary block |
| Mean score and mean path length split by normal vs flagged | in-model | summary block |
| Labelled CSV back out (original rows + score + flag) | in-model | `format = csv` keeps every input column and appends `anomaly_score`, `path_length`, `is_anomaly`, `rank` |
| Missing / non-numeric cell handling | in-model | `missing` = `drop` / `median` / `mean` / `zero` / `error` |
| Extended (hyperplane-split) isolation forest | in-model | `method = extended` |
| Header detection and mixed delimiters | in-model | `header` = `auto`/`yes`/`no`; comma, tab, semicolon, pipe and whitespace are auto-detected |
| Preset configurations / one-click sample | in-model | three `[[example]]` chips on the page |
| Score histogram, scatter plot of the top two features | **out of model** | The block returns text/JSON/CSV; this repo's chart-capable blocks (`data-clusterer`, `csv-chart-generator`) already own SVG plotting, and page `format` is text. Listed, not built. |
| File upload of CSV/XLSX | **out of model** | Pure blocks take pasted text; `xlsx-to-csv` already covers the spreadsheet leg of the pipeline. |
| Local outlier factor, one-class SVM comparison arms | **out of model** | Different estimators; they would make this a general outlier-method suite rather than an isolation forest. Deliberately out of scope so the page answers one question well. |
| An AI agent that narrates the result | **out of model** | Out of scope for a toolkit block. |

## Decisions and deviations

- **Score definition** follows the original paper and the reference implementation:
  `s = 2^(−E(h(x)) / c(ψ))` where `c(ψ) = 2·H(ψ−1) − 2(ψ−1)/ψ` is the average unsuccessful-search
  path length of a binary search tree of ψ nodes. Scores are in `(0, 1)`; higher means easier to
  isolate, i.e. more anomalous. `contamination = auto` therefore maps to the classic `0.5` cut-off,
  which is exactly what scikit-learn's `auto` offset of −0.5 means after the sign flip. Documented
  on the page so nobody has to guess which sign convention is in play.
- **No feature scaling parameter.** Axis-parallel isolation-forest splits are drawn uniformly
  between a feature's own min and max inside each node, so monotone per-column rescaling cannot
  change the tree structure or the scores. Adding a `scaling` control (as the SVM block has) would
  be a no-op switch that implies otherwise. The FAQ states this and notes the one exception —
  `method = extended`, whose hyperplanes do mix columns, so extended mode standardizes internally.
- **Ties at the contamination cut-off** flag every row at the threshold rather than truncating to
  exactly `round(p·n)` rows. The report prints both the requested rate and the realised count.
- **Row cap 20,000 / column cap 200.** Fitting is O(trees · ψ · log ψ) and scoring is
  O(n · trees · depth), so the cap is about keeping an interactive page responsive inside the
  64 MiB wasm sandbox, not about the algorithm.
- **Dropped rows stay in the CSV output** with empty score cells, so a `format = csv` result can be
  pasted straight back over the original table without the rows silently shifting.

## Sources

- <https://metricgate.com/docs/isolation-forest/>
- <https://scikit-learn.org/stable/modules/generated/sklearn.ensemble.IsolationForest.html>
- <https://orange3.readthedocs.io/projects/orange-visual-programming/en/latest/widgets/data/outliers.html>
- <https://arxiv.org/pdf/1811.02141>
- <https://en.wikipedia.org/wiki/Isolation_forest>

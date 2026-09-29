# hdbscan-cluster — competitor analysis (2026-09-23)

Scan run **before** implementing, per `create-next-tool`. All notes are paraphrased summaries of
publicly documented parameter surfaces and UX patterns. **No competitor copy, branding, wording or
trademark is reproduced anywhere in this repo.** Out-of-model items are listed, never built.

## Duplicate check (why this is a new block, not a rename)

`ls blocks/ | grep -iE 'cluster|dbscan|outlier|anomal'` →
`cluster-similar-values`, `data-clusterer`, `geo-cluster`, `iqr-outlier-trimmer`,
`isolation-forest-anomaly`, `outlier-detector`.

The only real overlap candidate is **`blocks/data-clusterer`**. Confirmed against
`blocks/data-clusterer/core/src/lib.rs` + `src/lib.rs`: its `method` enum is
`["kmeans", "dbscan", "hierarchical"]` and its density mode is plain DBSCAN, driven by a required
`eps` radius (`"dbscan needs eps > 0 (the neighbourhood radius)"`, `fn dbscan(points, eps,
min_samples)`), with a single global density scale and a binary core/noise decision.

HDBSCAN* is a different algorithm with a different user contract:

* **No `eps`.** It builds a mutual-reachability graph, a minimum spanning tree, and a condensed
  cluster tree, then selects clusters by **excess of mass over stability** — so clusters at
  *different* densities can coexist in one run, which single-scale DBSCAN cannot express.
* **Outputs DBSCAN has no analogue for:** per-point **membership probability**, per-point **GLOSH
  outlier score**, and per-cluster **persistence**.
* `data-clusterer` is chart-first (SVG scatter + PCA projection); this block is a text/JSON/CSV
  labelling and scoring report.

Same ruling class as `isolation-forest-anomaly` shipping alongside `outlier-detector`: a distinct
engine with distinct outputs, not a second name for a shipped code path. Not skiplisted.

## Competitors reviewed

| # | Competitor | What it is | Reached |
|---|---|---|---|
| 1 | scikit-learn `sklearn.cluster.HDBSCAN` | The reference implementation most users meet first | yes |
| 2 | `hdbscan` (scikit-learn-contrib) | The original McInnes/Healy library; richest attribute surface | yes |
| 3 | R `dbscan::hdbscan()` | The standard R implementation | yes |
| 4 | MetricGate DBSCAN calculator | A browser, no-code density-clustering calculator — included for **UX controls**, since 1–3 are code libraries and this block ships a web page | yes |

## Table-stakes matrix

Every row lands in the descriptor **or** in the "considered, not built" list below. Nothing dropped
silently.

### Parameters

| Capability | Seen in | Their default | Verdict | Our param |
|---|---|---|---|---|
| Minimum cluster size | 1, 2, 3 (`minPts`) | 5 | **in-model** | `min_cluster_size` (default 5) |
| Core-distance neighbourhood `k` | 1, 2 | `None` → mirrors min cluster size | **in-model** | `min_samples` (default 0 = mirror `min_cluster_size`) |
| Cluster-selection epsilon (merge micro-clusters below a distance) | 1, 2 | 0.0 | **in-model** | `cluster_selection_epsilon` (default 0) |
| Cluster selection method: excess-of-mass vs leaf | 1, 2 | `eom` | **in-model** | `selection` = `eom` \| `leaf` |
| Allow a single cluster as a valid answer | 1, 2 | false | **in-model** | `allow_single_cluster` (default false) |
| Max cluster size cap for excess-of-mass selection | 1 | `None` | **in-model** | `max_cluster_size` (default 0 = no cap) |
| Robust-single-linkage distance scaling `alpha` | 1, 2 | 1.0 | **in-model** | `alpha` (default 1.0) |
| Distance metric choice | 1, 2, 3 | euclidean | **in-model** (4 metrics) | `metric` = euclidean \| manhattan \| chebyshev \| cosine |
| Scale/standardize features before clustering | 4 ("Scale data", recommended on) | on | **in-model** | `normalize` (default true) |
| Pick which numeric columns to cluster on | 3, 4 | — | **in-model** | `features` (names or 1-based indices) |
| Missing-cell policy | 4 (implicit), general tabular practice | — | **in-model** | `missing` = drop \| median \| mean \| zero \| error |
| Parallelism / index knobs (`n_jobs`, `algorithm`, `leaf_size`, `approx_min_span_tree`, `core_dist_n_jobs`) | 1, 2 | various | **out-of-model** | single-threaded wasm; we always use the exact O(n²) Prim MST, so there is no index to choose and no approximation to toggle |
| `metric_params`, arbitrary callable metrics | 1, 2 | — | **out-of-model** | no user code execution in a wasm block |
| `prediction_data` / `approximate_predict` (label new points later) | 2 | false | **out-of-model** | each call is stateless; there is no fitted model to persist between runs |
| `cluster_selection_persistence`, `cluster_selection_epsilon_max` | 2 | 0.0 / inf | **considered, rejected** | narrow second-order knobs on top of an already 19-param surface; `cluster_selection_epsilon` + `max_cluster_size` cover the same "stop over-splitting / over-merging" intent |
| `branch_detection_data`, `gen_min_span_tree` | 2 | false | **considered, rejected** | they exist to feed follow-up plotting/analysis APIs this block does not expose |

### Outputs

| Output | Seen in | Verdict | Ours |
|---|---|---|---|
| Flat cluster label per point, noise marked | 1, 2, 3, 4 | **in-model** | `cluster` column; noise = `-1` (the scikit-learn/`hdbscan` convention, not R's `0`) |
| Membership probability / strength per point | 1, 2, 3 (`membership_prob`) | **in-model** | `probability` column (0 for noise) |
| GLOSH outlier score per point | 2, 3 (`outlier_scores`) | **in-model** | `outlier_score` column, for clustered *and* noise points |
| Per-cluster persistence / stability score | 2 (`cluster_persistence_`), 3 (`cluster_scores`) | **in-model** | `persistence` in the per-cluster summary and in JSON |
| Cluster count + noise count and share | 4 | **in-model** | summary header |
| Per-cluster size and per-feature means | 4 | **in-model** | per-cluster table + JSON `centroid` |
| Cluster centroid / medoid (`store_centers`) | 1 (`centroid`/`medoid`/`both`), 2 (`exemplars_`) | **in-model** | both reported per cluster in JSON; centroid means shown in the text summary |
| Machine-readable export of labels alongside the source rows | 4 (copyable results) | **in-model** | `format` = text \| json \| csv; CSV appends `cluster`, `probability`, `outlier_score` to the original row |
| k-NN distance plot, PCA scatter, condensed-tree/dendrogram plot | 2, 4 | **out-of-model here** | this block's page renders `format = "text"`; `blocks/data-clusterer` already owns the SVG scatter/PCA surface for tabular clustering, so duplicating a plotting engine here would fork that surface |
| DBCV / `relative_validity_` cluster-validity index | 2 | **considered, rejected** | a separate density-based validity computation over the MST with its own failure modes; per-cluster persistence already answers "is this cluster solid?" for the ranking use case. Revisit as a dedicated validity tool |
| Citation/reference export (APA, BibTeX, …) | 4 | **out-of-model** | account/manuscript workflow, not a browser-local compute block |

### UX controls (competitor 4 + the generator's current declarative kinds)

| Pattern | Verdict | How we ship it |
|---|---|---|
| Column/variable picker | in-model | `features` field, blank = every fully numeric column |
| Numeric fields with sane defaults surfaced in the UI | in-model | placeholders on every text/number field |
| Bounded numeric dials | in-model | `kind = "slider"` on `min_cluster_size`, `min_samples`, `max_cluster_size`, `top`, `decimals` |
| Friendly labels on fixed choices | in-model | `[input.labels]` on `metric`, `selection`, `missing`, `sort`, `header`, `delimiter`, `format` |
| Recommended-on scaling toggle | in-model | `normalize` boolean, default true (checked on load) |
| One-click worked presets | in-model | three `[[example]]` chips: two-blob default run, leaf-mode fine splitting, CSV export with imputation |
| Auto-parameter suggestion from a k-NN plot | out-of-model | requires the plotting surface above; the `min_samples = 0 → mirror min_cluster_size` default is the documented substitute |

## Design decisions taken from the scan

1. **Defaults match the reference implementation** — `min_cluster_size = 5`, `min_samples` mirroring
   it, `cluster_selection_epsilon = 0`, `selection = eom`, `alpha = 1.0`, `allow_single_cluster =
   false`, `metric = euclidean` — so results are comparable with what users already run elsewhere.
2. **`normalize` defaults to true**, following the browser calculator's "recommended" scaling
   default, because pasted tables mix units far more often than library inputs do. It is a visible
   checkbox and the summary always states whether it was applied.
3. **Noise is `-1`**, the majority convention (1, 2), and is stated on the page so R users are not
   surprised.
4. **Probabilities, GLOSH outlier scores and persistence ship from day one** — they are the reason
   to reach for HDBSCAN over the existing DBSCAN mode, so they are not a later enhancement.
5. **Deterministic, no seed param.** HDBSCAN* has no randomised step, unlike the sibling
   isolation-forest block; the summary says so instead of exposing a no-op knob.
6. **Row cap 5,000** (200 columns). The exact MST is O(n²); the cap is stated on the page rather
   than discovered through a hang.

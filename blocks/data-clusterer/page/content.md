## Cluster CSV data three ways, right in your browser

Paste a table, choose an algorithm, and see the groups your data falls into. **Data
Clusterer** runs **KMeans**, **DBSCAN**, and **hierarchical (agglomerative)** clustering
entirely in your browser — the CSV never leaves your machine, and there is no model to
download. The result is a self-contained **SVG scatter plot** coloured by cluster (with
centroid markers and a legend), a **labeled CSV** mapping every row to its cluster, or a
**JSON report** with cluster sizes, centroids, and a **silhouette** quality score.

- **KMeans** — partitions the rows into a fixed number of clusters (`k`) around their
  centroids. Deterministic farthest-point seeding means the same data always gives the
  same clusters.
- **DBSCAN** — finds clusters by density using `eps` (neighbourhood radius) and
  `min_samples`. Points in no dense region are labeled **noise** (drawn in grey) — great
  for spotting outliers.
- **Hierarchical** — repeatedly merges the closest clusters until the target count
  remains, with **average**, **complete**, **single**, or **Ward** linkage.

Feature columns are picked by header name or 1-based index — or auto-detected (every
fully-numeric column). Turn on **standardize** (z-score) so columns on different scales
(say, age vs. income) contribute fairly. Distance is Euclidean.

## Worked example

Input (KMeans, `k = 3`, standardized):

```
height,weight
150,52
158,55
162,58
170,70
174,73
178,76
185,92
189,96
193,101
```

KMeans groups the nine rows into three body-size segments. Switch **Output** to **JSON** to
read them back as data:

```json
{
  "method": "kmeans",
  "points": 9,
  "clusters": 3,
  "noise": 0,
  "normalized": true,
  "features": ["height", "weight"],
  "silhouette": 0.78,
  "cluster_detail": [
    { "cluster": 1, "size": 3, "centroid": [156.667, 55] },
    { "cluster": 2, "size": 3, "centroid": [174, 73] },
    { "cluster": 3, "size": 3, "centroid": [189, 96.333] }
  ]
}
```

The **silhouette** score runs from −1 to 1 — higher means tighter, better-separated
clusters, so you can compare different `k` values or methods.

## Reading the scatter plot

Each point is a row, coloured by its cluster; the larger diamond in each colour is that
cluster's centroid. With exactly two feature columns the axes are those columns. With more
than two features, the plot shows a 2-D **PCA projection** (PC1 / PC2) so the structure is
still visible — the clustering itself always uses all selected columns. A single feature is
plotted against its row number.

## Limits

- Clustering runs on up to 10,000 rows for KMeans/DBSCAN and 1,500 rows for hierarchical
  (which needs an n×n distance matrix); up to 50 feature columns.
- Distance is Euclidean; there is no Manhattan/cosine option (KMeans centroids assume
  Euclidean geometry).
- The silhouette score is reported for up to 4,000 clustered points.

## FAQ

<details>
<summary>Which algorithm should I choose?</summary>

Use **KMeans** when you already know roughly how many groups you want and they're
blob-shaped. Use **DBSCAN** when you don't know the count, the clusters may be irregular,
or you want outliers flagged as noise — tune `eps` and `min_samples`. Use **hierarchical**
for small datasets when you want control over how clusters merge (the **linkage**), or to
try several `k` values from one structure.

</details>

<details>
<summary>Should I turn on standardize (z-score)?</summary>

Usually yes. Standardizing rescales every feature to zero mean and unit variance, so a
large-magnitude column (e.g. income in the tens of thousands) doesn't dominate a
small-magnitude one (e.g. age). Turn it **off** when your columns are already on the same
scale and you want `eps` (for DBSCAN) to be interpreted in the raw units.

</details>

<details>
<summary>How do I pick DBSCAN's eps and min_samples?</summary>

`eps` is the maximum distance for two points to be neighbours; `min_samples` is how many
neighbours (including the point itself) a point needs to anchor a cluster. If everything
collapses into one cluster, lower `eps`; if everything is labeled noise, raise `eps` or
lower `min_samples`. With standardize on, `eps` is in standard-deviation units, so values
around 0.3–1.0 are a good starting range.

</details>

<details>
<summary>What is the silhouette score?</summary>

It measures how well each point sits in its cluster versus the nearest other cluster,
averaged over all points, from −1 to 1. Values near 1 mean tight, well-separated clusters;
near 0 means overlapping clusters; negative means points may be in the wrong cluster. It's
shown in the JSON output so you can compare settings objectively.

</details>

<details>
<summary>My data has more than two columns — how is it plotted?</summary>

Clustering always uses every selected feature column. For the scatter plot, two features
are drawn directly on the axes; with three or more features the tool computes a 2-D
**principal-component projection** (PC1 vs PC2) so you can still see the cluster structure.
Use the **CSV** or **JSON** output to get the exact per-row labels and centroids.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. All parsing, clustering, PCA, and rendering happen locally in your browser via
WebAssembly. Nothing is sent to a server and there is no model download.

</details>

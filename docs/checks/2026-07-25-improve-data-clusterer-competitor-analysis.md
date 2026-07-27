# data-clusterer — competitor analysis (2026-07-25)

Competitor scan run before implementing `data-clusterer` (Clusters tabular data with
KMeans, DBSCAN, or hierarchical methods and visualizes the result). All notes below are
**paraphrased** into original wording — no competitor copy, branding, or trademarks are
reproduced. Findings feed the descriptor + page; out-of-model items are listed, not built.

## Competitors surveyed (top 5 + 1 honorable mention)

1. **StatsKingdom — Cluster Analysis** (statskingdom.com/cluster-analysis.html) — KMeans only.
   Data grid with delimiter choices, optional ID column, drops non-numeric cells. Params: k
   (or k=0 → auto via elbow at 0.9 explained variance), scaling toggle (none/z-score), random
   restarts, max iterations, silhouette-threshold outlier flag. Output: colored scatter with
   centers, elbow curve, per-point silhouette table, outlier removal.
2. **ToolSlick — K-Means Calculator** (toolslick.com/programming/ml/kmeans) — KMeans with
   three seeding strategies (k-means++, random, PAM BUILD). Input: paste/file/URL/example,
   header toggle. Output: HTML table + CSV export.
3. **numiqo — Cluster Analysis Calculator** (numiqo.com/statistics-calculator/cluster) —
   KMeans + hierarchical + DBSCAN. Editable table, explicit numeric-variable selection,
   import/export. Guided 4-step flow; advertises client-side computation ("nothing sent to
   the cloud").
4. **CodingAce — Dendrogram Generator** (codingace.net/statistics/dendrogram_generator.html) —
   agglomerative hierarchical with single/complete/average(UPGMA)/Ward linkage; Euclidean or
   Manhattan; optional z-score; cut at k (2–12). Output: interactive dendrogram, merge-summary
   table, CSV/PDF export. Capped ~80 rows × 20 cols.
5. **Clustering Visualizer** (clustering-visualizer.web.app) — KMeans/DBSCAN/Mean-Shift/
   hierarchical, but canvas point-drawing (educational), animated steps, DBSCAN core/border/
   noise coloring. Not CSV-driven.

Honorable mention: Fournier-Viger DBSCAN demo — Eps + MinPts + core/border/noise coloring.

## Table stakes (targeted for parity)

- KMeans with a user-set `k` — **shipped**.
- Flexible tabular input (paste CSV, header handling) — **shipped** (auto header detection,
  quoted fields via the `csv` crate).
- Numeric column/variable selection, ignore non-numeric — **shipped** (`columns` by name or
  1-based index; blank = auto-detect every fully-numeric column; non-numeric rows skipped).
- Optional z-score standardization — **shipped** (`normalize`, default on).
- Colored 2D scatter output — **shipped** (self-contained SVG).
- Row→cluster label table + CSV export — **shipped** (`output=csv`); page has a Download link.
- Example/preset dataset — **shipped** (three `[[example]]` chips on the page).

## Differentiators (chosen to stand out — all in-model)

- **Three algorithms in one tool** (KMeans + DBSCAN + hierarchical) — only numiqo/visualizer
  span the set, and neither is a pure offline SVG tool. **Shipped.**
- **Hierarchical linkage choice** (average/complete/single/Ward via Lance–Williams). **Shipped.**
- **Silhouette quality score** in the JSON report. **Shipped.**
- **DBSCAN noise labeling** (grey points + "Noise (n=…)" legend entry). **Shipped.**
- **Centroid markers** drawn on the scatter for every cluster. **Shipped.**
- **Local PCA 2D projection** for datasets with >2 feature columns, so high-dimensional CSVs
  still get a meaningful scatter (PC1/PC2 axes) — pure linear algebra (deterministic power
  iteration + deflation), nothing downloaded. **Shipped.**
- **Privacy / offline story** — pure-Rust/wasm, runs entirely in the browser, no upload, no
  model download. **Shipped** (page copy states it).

## Considered, not built (rejected or out-of-model)

- **Auto-k via elbow method** (StatsKingdom): in-model but adds a second chart mode + heuristic
  threshold; deferred to keep the descriptor focused. The silhouette score already gives users a
  numeric signal to compare k values manually. *Considered, rejected for now.*
- **Manhattan / non-Euclidean distance metric**: KMeans centroids assume Euclidean geometry, so
  a metric switch would silently be wrong for KMeans. Euclidean only, stated on the page.
  *Rejected on correctness grounds.*
- **Interactive dendrogram / animation** (CodingAce, visualizer): needs a live DOM/canvas
  surface; the tool emits a static self-contained SVG. The flat cluster assignment at k is the
  in-model equivalent and is shipped. *Out-of-model for a static SVG.*
- **Multiple seeding strategies + random restarts** (ToolSlick/StatsKingdom): the tool uses a
  single deterministic farthest-point (k-means++ style, no RNG) seeding so results are stable
  and reproducible across the page/CLI/chat surfaces. Random restarts would break determinism.
  *Rejected to preserve determinism.*
- **URL/database dataset fetch, logins, saved projects, cloud/PDF export, AI "describe my data"**:
  need a server/account/model. *Out-of-model.*
- **t-SNE / UMAP projection**: heavy and slow in wasm; PCA is the safe local projection.
  *Out-of-model at scale.*
- Very large datasets: capped for browser responsiveness (50k parse rows; 10k points for
  KMeans/DBSCAN; 1.5k for hierarchical's n×n matrix) with actionable error messages.

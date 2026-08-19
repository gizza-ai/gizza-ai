# geo-cluster — competitor analysis (2026-08-06)

Scan run **before** implementing `blocks/geo-cluster`, per the create-next-tool recipe.
All competitor observations below are **paraphrased** from public documentation — no
competitor copy, branding, or trademarks are reproduced or reused, and nothing here is
copied into the tool's page.

## Scope of the pick

Backlog row: `geo-cluster — Clusters lat/long points using grid or DBSCAN and returns
cluster assignments and centroids.` (type hint `pure`).

### Duplicate check (not a duplicate)

`ls blocks/ | grep -iE 'geo|cluster|dbscan'` surfaces `data-clusterer`,
`cluster-similar-values`, `geofence-check`, `geojson-*`, `gpx-*`, `geometry-calculator`.
The only near miss is **`blocks/data-clusterer`**, which does offer a DBSCAN mode — but it
is a *generic tabular* clusterer:

- it clusters arbitrary numeric CSV columns in **Euclidean** space (verified in
  `blocks/data-clusterer/core/src/lib.rs`: `dbscan(&points, opts.eps, …)` over a plain
  `Vec<Vec<f64>>` feature space, optionally z-score standardized);
- its `eps` is therefore expressed in **feature units** — for lat/lon columns that means
  *degrees*, which is geographically wrong: one degree of longitude is ~111 km at the
  equator and ~28 km at 75° N, so a single degree-valued `eps` cannot express "within
  500 m" anywhere except along a meridian;
- it has no notion of latitude/longitude, no geographic centroid, no grid/geohash binning,
  and no geo input formats (GeoJSON, `lat,lon` rows).

`geo-cluster` is the geographic counterpart: a **haversine** metric with the radius given
in real ground distance (m/km/mi/…), a **latitude-compensated grid** mode, spherical
centroids, and geo-native input/output (GeoJSON in and out). Distinct metric, distinct
units, distinct I/O — built, not skiplisted. The page cross-references `data-clusterer`
conceptually (generic numeric clustering) without naming it as a competitor.

## Competitors reviewed (top 3 real tools)

The search space here is notable: there is essentially **no mainstream browser tool** that
does this — the field is databases, GIS servers, and libraries. That is the SEO/UX gap.

### 1. PostGIS `ST_ClusterDBSCAN` (spatial database window function)

- Signature is effectively *(geometry, eps, minpoints)* → an integer cluster id per input
  row, with **NULL for unclustered/noise** geometries.
- `eps` is in the units of the geometry's coordinate system, so meters only if the data is
  in a projected CRS; `minpoints` is the core-point neighbour count.
- Cluster count is discovered from the data, not requested (documentation contrasts this
  with its k-means sibling, which requires k up front).
- Documented caveat: a **border** geometry can sit within `eps` of core geometries in more
  than one cluster, and the assignment is then arbitrary unless the window is explicitly
  ordered — i.e. results are not deterministic by default.
- Related functions in the same family cover fixed-k clustering and simple
  within-distance grouping.

### 2. ArcGIS GeoAnalytics "Group By Proximity" (GIS server task)

- Groups features by a spatial relationship (intersects/touches/near, geodesic or planar)
  with a **search distance plus an explicit distance unit** (meters default; km, feet,
  miles, nautical miles, yards … all selectable).
- Output is a copy of the input with an added **`group_id`** field; features sharing a
  value are one group.
- Optional **temporal** proximity (a time window with its own unit) and an **attribute**
  expression, so features only group when they also match on time/attributes.
- Documented caveats: near-relationship analysis requires a projected coordinate system,
  and sliver features may drop out on tolerance. Deprecated in the newest release.

### 3. `geo-dbscan` (JS library) + the widely-copied scikit-learn haversine-DBSCAN recipe

- `geo-dbscan` takes **epsilon in meters** and a `minPoints` count, reads
  `{latitude, longitude}` off each record via a caller-supplied accessor, computes
  **haversine** distance, and uses **geohash bucketing as a spatial index** so neighbour
  lookups don't degrade to O(n²). Points not meeting the density criteria are marked noise.
- The scikit-learn recipe (Boeing's widely-cited spatial-data-reduction write-up and the
  many tutorials derived from it) is the de-facto reference: `DBSCAN(eps=<radius>/<earth
  radius>, min_samples=…, metric='haversine')` on **radian** coordinates, then a
  representative point / mean centroid per cluster.
- The companion `gps-cluster` script family takes an approximate cluster radius **in
  meters** and emits CSV with **centroid latitude, centroid longitude, and cluster
  membership** — the exact output shape this backlog row asks for.

## Table stakes → where each one landed

| Table stake (paraphrased) | Fit | Where it landed in gizza |
|---|---|---|
| Density clustering with a neighbourhood radius (`eps`) | in-model | `method = "dbscan"` + `radius` |
| Minimum neighbourhood size / core-point threshold | in-model | `min_points` (counts the point itself, as PostGIS/sklearn do) |
| Radius expressed in **real ground distance**, not degrees | in-model | haversine metric; `radius` is in `units` |
| **Selectable distance units** (m / km / mi / ft / nmi) | in-model | `units` enum (5 choices, meters default) |
| Grid / geohash binning as a cheaper alternative | in-model | `method = "grid"`, latitude-compensated cells |
| Cluster **id per input point** (assignments) | in-model | every output format carries a per-point cluster id |
| **Noise / unclustered** class (PostGIS NULL, DBSCAN noise) | in-model | `noise` in text/CSV, `null` in JSON, `"noise"` in GeoJSON |
| Cluster **centroids** | in-model | spherical (3-D unit-vector) mean per cluster |
| Cluster size + spread | in-model | `points`, `radius_<unit>` (max member→centroid distance), bbox |
| Spatial index so large inputs stay fast | in-model | both modes bin into a cell grid; DBSCAN scans the 3×3 cell halo |
| Deterministic, reproducible assignment | in-model | clusters numbered by first member in input order; border points go to the first claiming cluster in input order — no arbitrary tie-break (this is the PostGIS caveat, fixed) |
| CSV output for spreadsheets | in-model | `output = "csv"` |
| Structured output for scripting | in-model | `output = "json"` |
| GeoJSON in **and out** | in-model | GeoJSON accepted as input; `output = "geojson"` emits points + centroid features |
| Flexible input (CSV rows, JSON pairs/objects, GeoJSON) | in-model | one `points` field auto-detects all three |
| `lat,lon` vs `lon,lat` ordering | in-model | `coord_order` enum |
| Projected-CRS requirement / reprojection | **out-of-model** | we are haversine-on-WGS84 only; stated as a page limit instead of a knob |
| Fixed-k clustering (k-means, `ST_ClusterKMeans`) | **considered, rejected** | k-means over generic numeric features is already `blocks/data-clusterer`'s job; adding a geo k-means would fork that surface for little gain. Density + grid answer the "group nearby GPS points" question that this row asks. |
| Temporal proximity window (ArcGIS) | **considered, rejected** | needs a timestamp column and a second unit system; large schema cost for a niche case. Listed, not built. |
| Attribute-expression grouping (ArcGIS) | **out-of-model** | needs a general expression evaluator over arbitrary columns. |
| Hierarchical / OPTICS variants | **out-of-model** | O(n²) memory at the sizes a browser tool must hold; `data-clusterer` already offers hierarchical for small tabular data. |
| Server-side scale (millions of rows), accounts, stored layers | **out-of-model** | gizza is browser-local, no account, no backend. Input is capped at 10,000 points and the cap is stated on the page. |

## UX patterns worth matching (paraphrased, not copied)

- Competitors all make the **unit explicit** next to the distance — so `radius` and `units`
  sit next to each other, and every distance in the output is suffixed with the chosen unit.
- The GIS tools ship **preset relationships** rather than free-form config → the page ships
  `[[example]]` chips (a store-visit density example, a delivery-grid example, and a
  noise/outlier example) so a first-time visitor gets a real result in one click.
- Library docs lead with a worked snippet → the page leads with a full worked example whose
  exact output is reproduced on the page.
- Every competitor documents its noise/unclustered semantics prominently → the page states
  it in the copy, in the FAQ, and in the field help.

## Decisions recorded

1. **Haversine, not Euclidean.** The whole reason this tool exists next to `data-clusterer`.
   Mean Earth radius 6 371 008.8 m (IUGG).
2. **Grid mode compensates for latitude.** Cell height is a constant number of degrees; cell
   width is divided by `cos(latitude)` of the row's centre band, so cells stay roughly square
   on the ground instead of collapsing toward the poles.
3. **Determinism over speed.** Border-point ties resolve by input order (PostGIS's documented
   caveat becomes our guarantee).
4. **Spherical centroids.** 3-D unit-vector mean, so a cluster spanning the antimeridian gets
   the right answer instead of a mid-Pacific average of ±179°.
5. **10,000-point cap.** Keeps the wasm sandbox comfortable; stated on the page and in the
   error message rather than discovered by a hang.

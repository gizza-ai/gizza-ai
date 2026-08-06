Paste latitude/longitude points and choose DBSCAN or grid clustering to group nearby locations by real ground distance. The tool accepts CSV/TSV rows, JSON arrays, or GeoJSON, then returns cluster assignments and spherical centroids without sending data anywhere.

Example input:

```text
51.5074,-0.1278,Charing Cross
51.5079,-0.1281,Nelson's Column
51.5071,-0.1274,Strand
51.5155,-0.1410,Oxford Circus
51.5161,-0.1415,Regent Street
40.7128,-74.0060,New York
```

With `method=dbscan`, `radius=200`, `units=m`, and `min_points=2`, the London points split into nearby clusters and the New York point is reported as unclustered noise. Switch `output` to CSV for spreadsheet joins, JSON for full metadata, or GeoJSON for map layers.

DBSCAN is best when clusters can have irregular shapes and outliers should stay unclustered. Grid mode is deterministic and fast for rough tiling: each latitude-compensated cell becomes a cluster when it holds enough points.

Limits: up to 10,000 points, latitude must be -90..90, longitude -180..180, and radius must be positive. CSV headers, blank lines, and `#` comments are ignored. GeoJSON coordinates always use longitude, latitude order; `coord_order` only affects CSV rows and JSON numeric pairs.

<details>
<summary>When should I use DBSCAN instead of grid?</summary>

Use DBSCAN when "nearby" means points can chain together into natural shapes, such as store visits around a neighbourhood or GPS pings along a route. Use grid when you need reproducible buckets of a fixed size and do not want clusters to cross cell boundaries.
</details>

<details>
<summary>What radius should I choose?</summary>

Pick the distance at which two points should count as the same place. For store visits or venue dedupe, try 50-200 m. For city districts, try 1-5 km. The radius is a real ground distance computed with haversine math, not a raw degree threshold.
</details>

<details>
<summary>Why are some points marked as noise?</summary>

In DBSCAN, a point is noise when it is not within `radius` of a core point. A core point needs at least `min_points` neighbours including itself. Lower `min_points`, increase `radius`, or choose grid mode if every point must receive a bucket.
</details>

<details>
<summary>Can I put the result on a map?</summary>

Yes. Set `output=geojson` to get a FeatureCollection containing the original points tagged with cluster ids plus centroid features for each cluster. Paste that output into a GeoJSON viewer or GIS tool.
</details>

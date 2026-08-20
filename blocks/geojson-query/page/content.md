## About this tool

GeoJSON Query is a local, paste-and-run feature filter for GeoJSON files. It accepts a `FeatureCollection`, one `Feature`, a bare geometry, or an array of features, then applies a compact SQL-like query language without uploading data or needing PostGIS.

Use it for quick map-data questions such as:

- `SELECT name, population WHERE population > 100000 ORDER BY population DESC`
- `WHERE bbox(-90, 39, -88, 40)` to keep features whose bounding boxes intersect an area
- `WHERE contains(-89.5, 39.5)` to find polygon features containing a point
- `SELECT country, count(*) AS features GROUP BY country ORDER BY features DESC`

The query language supports `SELECT`, `WHERE`, `GROUP BY`, `ORDER BY`, `LIMIT`, and `OFFSET`. Property predicates include `=`, `!=`, `<`, `<=`, `>`, `>=`, `IN`, `LIKE`, `CONTAINS`, `STARTS_WITH`, `ENDS_WITH`, and `IS NULL`. Spatial helpers include `bbox()`/`intersects()`, `within()`, `contains(lon, lat)`, and `near(lon, lat, km)`.

All coordinates are interpreted as WGS84 longitude/latitude, matching RFC 7946 GeoJSON. The tool does not reproject data or implement full CQL2; it is designed for deterministic client-side filtering, projection, sorting, paging, and small aggregates.

## FAQ

<details>
<summary>What GeoJSON shapes can I paste?</summary>

You can paste a `FeatureCollection`, a single `Feature`, a bare GeoJSON geometry, or an array of `Feature` objects. Bare geometries are wrapped as one feature with empty properties so they can be queried with `$type` and spatial predicates.

</details>

<details>
<summary>What does the optional bbox field do?</summary>

The `bbox` field is a convenience prefilter written as `minLon,minLat,maxLon,maxLat`. It is ANDed with the query's own `WHERE` clause and uses intersects semantics, so a feature is kept if its computed bounding box overlaps the supplied box.

</details>

<details>
<summary>Can this replace PostGIS, CQL2, or a GIS engine?</summary>

No. It is intentionally a self-contained browser/CLI tool for one GeoJSON input. It does not reproject CRS values, perform polygon-polygon overlay, run joins, or implement full OGC CQL2. Use PostGIS, DuckDB Spatial, or a GIS engine for those heavier workflows.

</details>

<details>
<summary>Why do aggregate queries return JSON rows instead of GeoJSON?</summary>

Aggregates such as `count(*)` and `sum(population)` produce tabular rows, not map features. When a query contains aggregates or `GROUP BY`, `geojson` output automatically behaves like JSON rows; choose `csv` when you want spreadsheet-ready output.

</details>

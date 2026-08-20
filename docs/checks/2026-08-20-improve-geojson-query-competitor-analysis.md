# geojson-query — competitor analysis (2026-08-20)

Scan run **before** implementation, so the shipped descriptor already covers the table stakes.
All competitor notes are **paraphrased observations**; no copy, branding, or assets were reused.

## Competitors reviewed

| # | Competitor | Shape | What it does for "query a GeoJSON" |
|---|---|---|---|
| 1 | pg_featureserv query API (Crunchy Data docs) — *read in full* | Server (OGC API - Features) over PostGIS | The reference vocabulary for feature querying: `bbox=MINX,MINY,MAXX,MAXY` (intersects semantics, geographic lon/lat), plain `?<property>=<value>` equality filters, a `filter=` parameter taking CQL expressions with logical operators and spatial functions, `properties=` to project a subset of attributes, `limit=`/`offset=` paging with a server-enforced maximum, and `sortby=` with `+`/`-` prefixes for ascending/descending. CRS handling (`crs`, `bbox-crs`, `filter-crs`) is a first-class concern because the backend can reproject. Requires a database, a server process, and published tables. |
| 2 | Geokit `filter` command (Development Seed) — *read in full* | Open-source CLI over GeoJSON files | Three filter modes over a file: `by_properties` (key=value pairs), `by_properties_strict` (feature must carry *all* the named properties), and `by_geometry` (select by geometry type, e.g. only LineStrings). Output can be merged into one file or split per matched property / per geometry type. Notably it has **no spatial predicates at all** — no bbox, contains, intersects or within. Needs a Python install and file paths in/out. |
| 3 | `feature-filter-geojson` (Digital Democracy) — *read in full* | JS library, Mapbox GL filter spec | Filters are nested arrays: `["all", ["==","class","street_limited"], ["<=","admin_level",3], ["!=","$type","Polygon"]]`. Comparison (`==`, `!=`, `<`, `<=`, …), set membership (`in`), boolean combinators (`all`, `any`, `none`), and the magic `$type` key for geometry type. Compiles a filter into a predicate function; it does not aggregate, sort, page, or do anything spatial. Requires writing JSON-array expressions in code. |
| 4 | GeoServer OGC API - Features, Filtering extension (Part 3) — *from search result summary only* | Server | Adds CQL2 text/JSON filtering on top of the same OGC parameter set as (1): property predicates plus spatial predicates (`S_INTERSECTS`, `S_WITHIN`, …) against WKT literals. Same shape as (1): needs a running GeoServer with published layers. |
| 5 | ArcGIS Maps SDK GeoJSONLayer `definitionExpression` (Esri community thread) — *from search result summary only* | Commercial JS mapping SDK | Filters a loaded GeoJSON layer with a SQL-92-ish `WHERE`-style string (`POP > 100000`) and `FeatureFilter`/`FeatureEffect` for client-side geometry+attribute filtering. Confirms **SQL `WHERE` text is the expected authoring surface** for map users, but it only exists inside a map application, not as a standalone file tool. |

## Table stakes (all shipped in v0.1.0)

- Bounding-box filter with the OGC `minLon,minLat,maxLon,maxLat` ordering, intersects semantics (1, 4)
  — exposed both as a dedicated `bbox` parameter and as `bbox(...)` inside `WHERE`.
- Property equality plus the full comparison set `= != < <= > >=` (1, 3, 5).
- Boolean combination with `AND` / `OR` / `NOT` and parentheses (1, 3, 4).
- Set membership `IN (…)` / `NOT IN (…)` (3).
- Geometry-type filter — we accept `$type` exactly as (3) spells it, and `geometry_type` as a synonym (2, 3).
- Property projection: `SELECT name, population` keeps only those properties (1's `properties=`).
- `ORDER BY <field> [ASC|DESC]`, `LIMIT n`, `OFFSET n` (1, 5).
- Spatial predicates beyond bbox: point-in-feature `contains(lon, lat)` and full containment
  `within(minLon, minLat, maxLon, maxLat)` (4).
- Output stays a valid GeoJSON `FeatureCollection` so the result drops straight back into a map (2).

## Gaps closed relative to the field (our differentiators)

- **One SQL-ish string that does filter + project + sort + page + aggregate.** (1)/(4) need a server,
  (2) has no spatial support and no sorting, (3) has no SELECT/ORDER/LIMIT at all, and (5) is locked
  inside a paid mapping SDK. Here it is one paste-and-run page, one CLI call, one chat tool.
- **Aggregation — nobody in the field has it.** `COUNT(*)`, `SUM`, `AVG`, `MIN`, `MAX` with optional
  `GROUP BY` and `AS` aliases, so "how many features per country, inside this bbox" is one query
  instead of an export into another tool. That is the "sedona-style spatial SQL on a single file"
  idea the backlog row asked for.
- **`near(lon, lat, km)`** great-circle proximity — asked for constantly in map work; (1)/(4) need
  a PostGIS `ST_DWithin` round trip, (2)/(3) can't do it.
- **Text predicates `LIKE` (with `%`/`_`), `CONTAINS`, `STARTS_WITH`, `ENDS_WITH`, `IS NULL`.**
  Only CQL (1, 4) has `LIKE`, and it needs a server.
- **Three output shapes from one query** — `geojson` (FeatureCollection), `json` (property rows),
  `csv` (spreadsheet-ready) — versus (2)'s file-splitting-only output model.
- **`missing_properties = false` semantics are documented, not implicit**, and unknown property
  names are *not* an error (a heterogeneous FeatureCollection is normal) — while
  `by_properties_strict` (2) makes that an all-or-nothing mode.
- **Stated, enforced caps** (8 MB input, 200,000 features) with actionable error text, instead of
  (1)'s silent server-side `LimitMax`.
- **Runs entirely locally** — no upload, no database, no API key, no install for the page.

## Considered, not built (out of model or rejected)

- **CRS / reprojection (`crs`, `bbox-crs`, `filter-crs`, SRID transforms)** (1, 4) — needs a PROJ
  datum/grid database; way outside a self-contained wasm block. All coordinates are treated as
  WGS84 lon/lat, which is what GeoJSON (RFC 7946) mandates anyway, and the page says so.
- **Full CQL2 grammar** (4) — temporal operators, array operators, function calls, `CASEI`/`ACCENTI`.
  We ship the SQL-ish subset that covers the common cases and document the grammar exactly; a
  half-implemented CQL2 that silently mis-parses would be worse than an honest subset.
- **Arbitrary-polygon spatial predicates (`S_INTERSECTS(geom, POLYGON((…)))`)** (4) — `contains()`
  here tests a point against the *feature's own* polygon, and `within()` takes a bbox. A general
  polygon-vs-polygon intersection/clipping engine is a different tool; `geofence-check` already
  covers "which of these points are inside this polygon".
- **Joins across two collections, spatial joins** (1) — single-input tool by construction.
- **Writing back to a database / tile serving / paging links (`next`/`prev` hrefs)** (1, 4) — server
  concerns; a single request here returns the whole result.
- **Per-match output file splitting** (2) — the page and CLI emit one document; `geojson-merge`
  and the shell cover the split/merge side.
- **Interactive map preview of the result** (5) — the shared page renderer is text + download; a
  slug-specific map widget would be a hack in the generic runtime. `geojson-to-svg` renders a
  static preview.
- **Editing / mutating properties (UPDATE, SET, computed columns)** — this tool is read-only by
  design; `geojson-format` handles property keep/drop and precision rounding.

## Verification snapshot

Built and verified on 2026-08-20: `cargo test --workspace` (core unit + drift-guard),
`scripts/build-block-wasm.sh geojson-query`, `wasm-pack` web build, manifest sync,
`gizza tool geojson-query` exact-output CLI runs, Playwright page spec (including a `?param=` deep
link and real wasm output assertions), repo JavaScript tests, and `scripts/check-tool-hygiene.py
geojson-query`.

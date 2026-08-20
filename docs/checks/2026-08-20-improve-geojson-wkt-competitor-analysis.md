# geojson-wkt — competitor analysis (2026-08-20)

Scope: one web search (`GeoJSON to WKT WKB converter online tool`), top results skimmed before
implementation. All notes below are PARAPHRASED observations of behaviour/feature surface — no
competitor copy, branding or trademarks are reproduced or reused.

## Scanned

| # | Tool (paraphrased role) | What it does |
|---|---|---|
| 1 | Browser-local "WKB/WKT/GeoJSON brew" converter | Multi-format hub: WKB, WKT, GeoJSON, KML in a single paste box, plus a map preview. Format is auto-detected from the pasted text. |
| 2 | Map-editor suite with per-pair converter pages (GeoJSON→WKT page) | Dedicated one-way pages, each with an Options panel; explicitly states standard WKT carries no SRID and that it does not reproject; points users at EWKT / an EWKB-enabled sibling page when an SRID is needed. Multi-feature default is a single `GEOMETRYCOLLECTION`; an option switches to one WKT per feature. Accepts Geometry, Feature, FeatureCollection, GeometryCollection. Runs fully client-side. |
| 3 | Generic file-conversion site (GeoJSON→WKT page) | File drag-and-drop + bulk convert + zip download. No options, no examples, no FAQ; free tier is metered behind a paid pass. |
| 4 (context) | Map-visualiser converter | Paste + "convert & visualise"; FAQ is definitional ("what is GeoJSON", "is install needed"). No WKB, no SRID, no examples. |

## Table stakes (observed across the field)

1. **Both directions.** GeoJSON→WKT and WKT→GeoJSON are the two most-linked pages everywhere.
2. **Accept every GeoJSON envelope** — bare Geometry, Feature, FeatureCollection, GeometryCollection.
3. **All 7 OGC geometry types** — Point, LineString, Polygon, MultiPoint, MultiLineString,
   MultiPolygon, GeometryCollection.
4. **Multi-feature policy is an explicit choice**, not a hidden default: collapse into one
   `GEOMETRYCOLLECTION` vs emit one WKT per feature (newline-separated).
5. **WKB as hex** is the third format in the serious tools; **EWKB/EWKT with an SRID** is the
   documented answer for "WKT has no SRID".
6. **Auto-detect input format** — users paste whatever they have.
7. **Empty geometry handling** (`POINT EMPTY`, `GEOMETRYCOLLECTION EMPTY`) — WKT round-trips need it.
8. **Runs locally / nothing uploaded** is a stated selling point on the good tools.
9. **Coordinate precision control** — mentioned as a gap on most (nobody in the top 3 ships it).

## Defaults chosen here (and why)

| Decision | Value | Rationale |
|---|---|---|
| `to` | `wkt` | The dominant request direction; WKB is opt-in. |
| `from` | `auto` | Matches competitor #1's paste-anything UX; explicit override kept for ambiguous input. |
| `multi` | `collection` | Same default as competitor #2 (one output string, lossless nesting). |
| `srid` | `0` (omit) | Standard WKT/WKB has no SRID; adding one silently would produce EWKT nobody asked for. |
| `precision` | `-1` (shortest round-trip) | Rust's shortest round-trip float formatting is exact and stable; rounding is opt-in. |
| `wkb_encoding` | `hex` | What PostGIS `ST_AsBinary`/`::text` shows; base64 offered for JSON transport. |
| `wkb_endian` | `little` (NDR) | What PostGIS emits. |
| `pretty` | `true` for GeoJSON output | Output is meant to be read/pasted. |

## Gaps we CLOSE (in model)

- Both directions in ONE tool + a third format (WKB) — the top 3 each split these across pages.
- **EWKT (`SRID=4326;POINT(…)`) and EWKB** (SRID-prefixed, high-bit flags) read AND write.
- **Z / M / ZM dimensions**: `POINT Z (1 2 3)`, `POINT M`, `POINT ZM`, and the PostGIS
  `POINTZ`/3D-EWKB flag forms — round-tripped to/from GeoJSON's 3rd coordinate ordinate.
- `EMPTY` geometries in every type.
- **Coordinate precision** (`precision=N` decimals) — nobody in the top 3 offers it.
- **base64 WKB** in addition to hex (nobody offers it) — practical for JSON/env transport.
- Big/little endian WKB (XDR/NDR) — competitor tools emit NDR only.
- Worked examples on the page as one-click chips; ≥3 FAQ accordions; explicit limits.

## Out of model (documented, NOT built)

- **Map preview / draw-on-map editing** — needs a tile-serving map component; gizza pages are
  static, offline, single-widget.
- **Reprojection between CRS** (EPSG transforms) — needs the PROJ database; the SRID here is
  carried as metadata only, coordinates are never transformed. Stated on the page.
- **Shapefile / GeoPackage / KML** — separate container formats; `kml-to-geojson`,
  `shapefile-to-geojson` and `topojson-to-geojson` already exist as their own blocks.
- **File upload / bulk zip conversion** — this is a paste-text pure block; the CLI covers batch.
- **Property preservation into WKT** — WKT is geometry-only by definition; feature properties are
  dropped by design (documented in the FAQ) rather than smuggled into a side channel.

## Not a duplicate

`ls blocks | grep -iE 'geo|wkt|wkb'` → the existing geo blocks are container/CSV converters
(`geojson-to-csv`, `geojson-coords-to-csv`, `csv-to-geojson`, `kml-to-geojson`,
`shapefile-to-geojson`, `topojson-to-geojson`, `gpx-to-geojson`), formatters (`geojson-format`),
a validator (`geojson-validate`), an SVG renderer (`geojson-to-svg`) and analysis blocks
(`geofence-check`, `geo-cluster`, `geometry-calculator`). Only `geojson-to-csv` mentions WKT, and
only as an optional CSV *column* while flattening features into a table — it cannot read WKT, has
no WKB, no SRID/EWKT and no GeoJSON output. No block converts WKT/WKB → GeoJSON at all.

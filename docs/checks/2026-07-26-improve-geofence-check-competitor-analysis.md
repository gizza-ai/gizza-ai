# geofence-check — competitor analysis (2026-07-26)

Point-in-polygon / geofence checker: test whether latitude/longitude points fall
inside a polygon boundary. Scan done BEFORE implementation to fix the in-model shape.

## Competitors skimmed

1. **Mapscaping — Point In Polygon Checker** (mapscaping.com/point-in-polygon-checker/)
   — browser tool. Three point-input modes: manual lat/lng add, paste CSV (`lat,lng`
   one per line), upload `.geojson`/`.kml`/`.gpx` of point features. Draw/import a
   polygon on a map. Results panel = summary (total / inside / outside) + a table
   with point #, lat, lon, status. "Download CSV" export with columns
   `point, latitude, longitude, status`. Decimal degrees only (lat −90..90, lon
   −180..180). All processing local, nothing uploaded.
2. **Turf.js `booleanPointInPolygon`** (turfjs.org; also exposed as an MCP tool) —
   library standard. Takes a GeoJSON Point + a GeoJSON `Polygon`/`MultiPolygon`,
   returns a boolean. Key option: `ignoreBoundary` — whether a point exactly on the
   edge counts as inside (default: boundary counts as inside). Handles polygon holes
   (interior rings) and MultiPolygon natively. This is the de-facto correctness ref.
3. **Shapely / GIS `point_in_polygon`** (glama.ai gis-mcp; Towards-Data-Science
   geofencing walkthroughs) — `polygon.contains(point)` vs `.touches()`/`.intersects()`
   distinguishes strictly-inside from on-boundary. Ray-casting / even-odd is the
   textbook algorithm; holes handled by ring parity. Input is WKT or GeoJSON.

## Table-stakes (paraphrased, not copied)

- Test **many points at once** against one polygon, per-point inside/outside status.
- Accept **GeoJSON** polygons: `Polygon`, `MultiPolygon`, `Feature`, `FeatureCollection`,
  **with holes** (interior rings punch out).
- Accept a **simple/CSV** coordinate form for both polygon and points (one `lat,lon`
  per line) — not everyone has GeoJSON.
- **Boundary handling** is an explicit choice (on-edge point → inside vs outside vs
  reported as its own "boundary" status) — this is Turf's `ignoreBoundary` and
  Shapely's contains-vs-touches split.
- **Summary counts** (total / inside / outside) plus a per-point table.
- **CSV + JSON export** of results; columns point #, latitude, longitude, status.
- Decimal-degree validation (lat −90..90, lon −180..180) catches swapped lat/lon.
- Fully **local / offline**, deterministic.

## Defaults chosen

- Coordinate order for the **simple/CSV/JSON-pair** forms: `lat,lon` (matches
  Mapscaping's CSV and how people speak coordinates). GeoJSON is always `[lon,lat]`
  per RFC 7946 and ignores the order setting.
- Boundary handling default: **inside** (a point on the edge counts as inside — Turf's
  default with `ignoreBoundary:false`).
- Output default: **text** (summary + per-point lines); `csv` and `json` for export.

## In-model (built here)

- `polygon`: GeoJSON `Polygon`/`MultiPolygon`/`Feature`/`FeatureCollection` (holes +
  multipolygon supported), OR a simple ring as `lat,lon` CSV lines / a JSON array of pairs.
- `points`: CSV `lat,lon[,label]` lines, a JSON array of pairs / `{lat,lon,label}`
  objects, or GeoJSON Point/MultiPoint/Feature/FeatureCollection.
- `coord_order` enum (`lat_lon` default / `lon_lat`) for the non-GeoJSON forms.
- `boundary` enum (`inside` default / `outside` / `boundary`).
- `output` enum (`text` default / `csv` / `json`) — CSV columns point,latitude,longitude,status.
- Ray-casting even-odd with explicit on-edge detection; deterministic, offline.
- Degree-range validation with a "did you swap lat/lon?" hint.

## Out-of-model (NOT built — needs a map/model/network, out of scope for a pure block)

- Interactive map to draw the polygon or plot points.
- `.kml` / `.gpx` file upload parsing (GeoJSON covers the structured case).
- Reprojection between CRS, nearest-boundary distance, buffering.

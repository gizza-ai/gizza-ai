# csv-to-geojson — competitor analysis (2026-08-18)

Scan run before completing the tool, per `/create-next-tool` and `/improve-tool`. Everything below is paraphrased; no competitor copy, branding or trademarks are reused.

Search: "CSV to GeoJSON converter online latitude longitude points line polygon" (WebSearch, 2026-08-18). The result set was a mix of browser converters, geospatial format hubs, and open-source CSV-to-GeoJSON pages. Three reachable, real references were used for table stakes.

## Profiles

### 1. bbk.co CSV to GeoJSON converter — online point converter

```json
{
  "name": "CSV to GeoJSON online",
  "url": "https://bbk.co/convert/csv-to-geojson",
  "features": [
    "turns CSV rows with latitude/longitude columns into GeoJSON Points",
    "keeps non-coordinate columns as properties",
    "skips invalid coordinates",
    "supports latitude/longitude or WKT mapping",
    "file upload and .geojson download flow"
  ],
  "params_options": [
    {"name": "coordinate mapping", "type": "column selection", "default": "lat/lon style", "range": "lat/lon or WKT"}
  ],
  "input_formats": ["CSV file"],
  "output_formats": ["GeoJSON download"],
  "ux_patterns": ["file upload", "download result", "preserve properties"],
  "limits": ["no hard row limit visible from search result"],
  "free_vs_paid": "free web tool"
}
```

### 2. GeoUtil converters — client-side geospatial converter hub

```json
{
  "name": "Geo Format Converters",
  "url": "https://geoutil.com/converters/",
  "features": [
    "CSV latitude/longitude rows to GeoJSON Point features",
    "auto-detects common headers such as lat/lon and latitude/longitude",
    "client-side processing privacy claim",
    "sits alongside other geospatial format converters"
  ],
  "params_options": [
    {"name": "lat/lon columns", "type": "auto-detect or choose", "default": "common header auto-detect", "range": "lat/lon aliases"}
  ],
  "input_formats": ["CSV"],
  "output_formats": ["GeoJSON"],
  "ux_patterns": ["privacy/client-side emphasis", "format converter hub", "auto-detection"],
  "limits": ["point conversion only in result description"],
  "free_vs_paid": "free web tool"
}
```

### 3. Open Innovations CSV2GeoJSON — open-source mapping converter

```json
{
  "name": "CSV to GeoJSON converter",
  "url": "https://open-innovations.github.io/CSV2GeoJSON/",
  "features": [
    "creates GeoJSON from valid CSV",
    "supports latitude/longitude columns",
    "also supports UK geography identifiers such as OS grid references and administrative areas",
    "open-source browser page"
  ],
  "params_options": [
    {"name": "geometry source", "type": "choice", "default": "latitude/longitude", "range": "lat/lon, OS grid, statistical/geography codes"}
  ],
  "input_formats": ["CSV"],
  "output_formats": ["GeoJSON file"],
  "ux_patterns": ["mapping/source-mode choices", "download output"],
  "limits": ["country-specific lookup modes require built-in reference data"],
  "free_vs_paid": "open source / free"
}
```

## Table stakes → in-model / out-of-model

| Table stake | Verdict | Where it lands |
| --- | --- | --- |
| Convert CSV rows with lat/lon columns to GeoJSON Points | in-model | default `shape=points` FeatureCollection |
| Preserve other columns as properties | in-model | non-coordinate fields copied to `properties` |
| Auto-detect common coordinate headers | in-model | lat/lon alias lists and numeric prefix sanity checks |
| Let users specify columns manually | in-model | `lat`, `lon`, and `elevation` column name or 1-based index |
| Support common delimiters | in-model | auto, comma, semicolon, tab, pipe |
| Keep processing local/private | in-model | pure Rust + wasm page, no network calls |
| Skip invalid coordinate rows | in-model | `invalid=skip` default |
| Error or preserve invalid rows for QA | in-model extension | `invalid=error` and `invalid=null` |
| Output compact or pretty GeoJSON | in-model | `pretty` boolean |
| Add bbox | in-model | `bbox` boolean |
| Generate lines or polygons from row order | in-model extension | `shape=line` and `shape=polygon` |
| Accept JSON row arrays | in-model extension | JSON array/object wrapper parser |
| WKT geometry columns | out-of-model for this block | would require parsing arbitrary geometry, not just tabular coordinates |
| Shapefile/KML/TopoJSON conversion | out-of-model | separate format-specific tools already exist or belong in other blocks |
| OS grid/admin-code geocoding | out-of-model | requires reference datasets/lookups, not a pure local coordinate table converter |
| Address geocoding | out-of-model | needs network/API or bundled geocoder data |
| CRS reprojection | out-of-model | requires projection libraries and explicit CRS inputs; this tool assumes decimal WGS84 lat/lon |

## Decisions

- Default behavior matches the broad competitor baseline: auto-detected latitude/longitude CSV to a Point FeatureCollection.
- The descriptor exposes every meaningful converter control: delimiter, column mapping, shape, property typing, precision, invalid-row policy, bbox, and pretty output.
- JSON rows are accepted as a convenience because many developer workflows already hold records as objects.
- The page explains GeoJSON coordinate order (`[longitude, latitude]`) because this is a common source of user mistakes.
- WKT, shapefile, geocoding, and reprojection were intentionally left out rather than half-implemented; they need different models or heavier geospatial dependencies.

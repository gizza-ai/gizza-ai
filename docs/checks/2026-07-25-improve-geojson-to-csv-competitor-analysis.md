# geojson-to-csv — competitor analysis (2026-07-25)

Function: flatten GeoJSON FeatureCollections, Features, or geometries into CSV rows for spreadsheets, GIS cleanup, and data pipelines. Findings are paraphrased from public tools/docs; no competitor copy, branding, or trademarks are reused.

## Competitors surveyed

| Tool | Core behavior | Notable options | Notes |
|---|---|---|---|
| Mapshaper | Converts GeoJSON to CSV/table-oriented exports | Keeps properties, can run commands, strong geometry processing | Professional GIS workflow; geometry handling is rich but CLI-like |
| MyGeodata Cloud converter | GeoJSON upload to CSV download | Projection/file conversion options, zipped outputs | Broad file-conversion UX, not a lightweight paste-and-run tool |
| Aspose GIS converter | GeoJSON to CSV online | Upload/download flow, batch-ish file conversion | Table-stakes conversion with minimal controls |
| ConvertCSV / JSON-to-CSV style tools | Generic JSON/GeoJSON flattening to CSV | Flatten nested objects, delimiter/header options | Good property-flattening UX but often treats geometry as ordinary JSON |
| geojson2csv / npm libraries | CLI/library flattening | Select fields, flatten properties, geometry columns | Developer-oriented; useful defaults are properties + lon/lat or WKT |

## Table-stakes distilled

- Accept **FeatureCollection** and single **Feature** inputs. → in-model.
- Produce one CSV row per feature and union property keys across rows. → in-model.
- Include geometry in a spreadsheet-friendly way. Competitors commonly use coordinates or WKT. → in-model as `geometry = wkt | lonlat | both | none`.
- Preserve nested property data instead of dropping it. → in-model as `nested = json | flatten`.
- Let users choose common separators and whether to emit headers. → in-model as `delimiter` enum and `header` checkbox.
- Quote CSV fields correctly when values contain commas, quotes, or newlines. → in-model.

## In-model decisions shipped

- `geojson`: accepts FeatureCollection, Feature, bare geometry, or an array of those.
- `geometry`: `wkt` default for lossless 2-D geometry text; `lonlat` for first-coordinate spreadsheet columns; `both`; `none`.
- `nested`: `json` default keeps nested objects/arrays compact; `flatten` expands dot-notated leaf columns.
- `delimiter`: comma, semicolon, tab, or pipe.
- `header`: default true, with a non-default no-header state.
- Page UX: textarea for GeoJSON, select controls for enum params, checkbox for header, preset examples for common conversions, and deep-linkable params.

## Out-of-model / deferred

- Coordinate reprojection, CRS transforms, simplification, joins, and topology repair are GIS operations outside this small pure converter.
- DBF/Shapefile, GeoPackage, and zipped multi-file export are file-format conversion tasks rather than paste-to-CSV.
- Selecting/reordering arbitrary columns is useful but deferred; the current first-seen union order is predictable and covers the common case.

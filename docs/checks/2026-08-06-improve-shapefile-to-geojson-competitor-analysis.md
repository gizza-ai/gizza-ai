# shapefile-to-geojson — competitor analysis (2026-08-06)

## Function under study

Convert an ESRI shapefile set into GeoJSON. A shapefile is a set of binary sidecar files rather than one text file: `.shp` carries geometry, `.dbf` carries attributes, `.prj` carries CRS metadata, `.cpg` may declare DBF encoding, and `.shx` is an index. Most public datasets ship those together as a `.zip`.

## Duplicate / viability check

Existing GeoJSON tools convert, merge or analyze already-GeoJSON inputs; `dbf-table-parser` parses the attribute table only. No existing block parses `.shp` records, pairs them with `.dbf` attributes, and emits GeoJSON. The standalone page model is not a fit because this is a binary multi-file upload/archive converter, so this ships as a chat/CLI file tool with `url`/`ref` input and no page.

## Competitors reviewed

### 1. Mapshaper-style converters

- Common shape: upload a shapefile zip, inspect layers, simplify or transform geometry, export GeoJSON/TopoJSON.
- Table stakes: zipped shapefile set input, layer selection when several `.shp` files are present, GeoJSON output, attribute preservation, coordinate precision controls.
- In-model decisions: accept a `.zip` or bare `.shp`, deterministic `layer` choice, preserve `.dbf` properties, `columns` subset/reorder, `precision`, and NDJSON for streamable output.
- Out of model: browser editing UI, topology-preserving simplification and projections are larger GIS operations and not part of a local file converter block.

### 2. GDAL/ogr2ogr-style CLI conversion

- Common shape: `ogr2ogr -f GeoJSON out.geojson in.shp` with many driver options.
- Table stakes: robust shape-type support, layer selection, bounding box, attribute table decoding, warnings for CRS/projection issues, feature limits for previews.
- In-model decisions: parse Point/MultiPoint/PolyLine/Polygon and Z/M variants, reject MultiPatch explicitly, emit bbox by default, report `.prj` name, warn when a projected CRS needs reprojection before web-map use, support `limit`.
- Out of model: arbitrary reprojection, spatial SQL, clipping and driver-specific creation options.

### 3. Online shapefile-to-GeoJSON converters

- Common shape: drag a shapefile zip into a web form and download a `.geojson` file.
- Table stakes: zip handling, attribute preservation, clear file-size limits, downloadable output, actionable errors when the archive lacks `.shp`.
- In-model decisions: zip member limit, clear archive/layer errors, output filename derived from the layer, downloadable `data:` result in the UI envelope.
- Out of model: a standalone browser page in this repo; the current page generator is text/field-oriented and this converter requires binary archive upload plus CLI/chat attachment plumbing.

### 4. GeoJSONL / data-engine workflows

- Common shape: stream one feature per line into tools such as tilers or SQL engines.
- Table stakes: FeatureCollection output for normal use, newline-delimited Feature output for pipelines.
- In-model decision: `output=geojson|ndjson`.

## Gap list → decisions

| Capability | Fit | Decision |
| --- | --- | --- |
| Zipped shapefile set | in-model | Built; picks a `.shp` and matching sidecars by shared stem. |
| Bare `.shp` | in-model | Built with empty properties and a warning. |
| `.shx` requirement | in-model | Not required; `.shp` is sequential-readable and `.shx` is an index. |
| Point, MultiPoint, PolyLine, Polygon | in-model | Built, including Z/M variants; M dropped because GeoJSON has no measure slot. |
| MultiPatch | in-model as rejection | Explicit actionable error: no GeoJSON geometry equivalent. |
| DBF attributes | in-model | Built via existing DBF parser core, including selected columns and encoding. |
| CRS reporting | in-model | Built; `.prj` name reported and projected CRS warning emitted. |
| Reprojection to EPSG:4326 | out-of-model | Not built; requires a projection database/engine. |
| Simplification/topology repair | out-of-model | Not built; belongs in a dedicated GIS editing/conversion tool. |
| Layer selection | in-model | Built with `layer`. |
| Coordinate rounding | in-model | Built with `precision` -1..17, default 6. |
| Feature limit / preview | in-model | Built with `limit`. |
| GeoJSONL output | in-model | Built with `output=ndjson`. |
| Standalone generated page | out-of-model for this repo | Not built; binary archive + multi-file semantics are exposed through file `url`/`ref` CLI/chat surface. |

## Copy / UX notes

- Lead with “upload/pass the zip”, not just `.shp`, because `.dbf` sidecars are where attributes live.
- State `.shx` is not required to avoid a common user blocker.
- Warn clearly about projected CRS: conversion is not reprojection, so metres will not line up on web maps until transformed to WGS84.
- Use downloadable GeoJSON/GeoJSONL output and keep the LLM summary short for large boundary files.

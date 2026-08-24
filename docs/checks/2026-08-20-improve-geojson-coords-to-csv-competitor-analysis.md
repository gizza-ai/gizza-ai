# geojson-coords-to-csv competitor analysis — 2026-08-20

## Scope

Tool: `geojson-coords-to-csv` — extract every coordinate position from GeoJSON into a flat CSV with one row per coordinate.

Search query used: `GeoJSON coordinates to CSV extract vertices lon lat online tool`.

## Competitors reviewed

| Competitor | Observed table-stakes behavior | In model? | Decision for this tool |
| --- | --- | --- | --- |
| GeoUtil Coordinate Extractor | Browser-style coordinate extraction across Point, LineString, Polygon, MultiPolygon, and GeometryCollection; optional feature properties; lat/lon or lon/lat ordering; coordinate indexing; no upload positioning. | Yes | Implement all core pieces: all geometry traversal, `properties`, `order`, `columns=indexed/feature/full`, local processing copy. |
| GeoJason GeoJSON to Lat/Long Extractor | Simple paste-to-CSV flow focused on Point geometries and latitude/longitude columns. | Yes | Include a `shapes=points` filter and `order=latlon` for map/spreadsheet workflows, while supporting more geometry types than point-only tools. |
| MyGeodata Cloud GeoJSON to CSV | General GIS file converter with upload workflow and broader format/transformation support, including CSV output from GeoJSON. | Partly | In-model: CSV output and common coordinate/property columns. Out-of-model: cloud upload pipeline, shapefile/KML/DXF conversions, CRS transformations. |
| Honeycomb Maps GeoJSON to CSV Converter | Feature-level GeoJSON-to-CSV conversion with coordinate encodings or lat/lon for points. | Partly | Clarify this tool is coordinate-level rather than feature-level; include properties and feature context columns so users can relate vertices back to features. Out-of-model: WKT/WKB geometry encoding because this backlog item is specifically vertex extraction. |

## Table-stakes checklist

| Capability / UX pattern | In model? | Implemented as |
| --- | --- | --- |
| Paste a GeoJSON document directly | Yes | Required `geojson` textarea with worked examples. |
| Accept FeatureCollection, Feature, bare geometries, and arrays | Yes | Core parser walks all supported GeoJSON geometry forms. |
| Traverse Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, GeometryCollection | Yes | Recursive geometry walker, plus `shapes` filter. |
| One CSV row per coordinate/vertex | Yes | `convert_str` emits every position in document order. |
| Choose lon/lat or lat/lon column order | Yes | `order=lonlat|latlon` enum with page labels. |
| Include headers and coordinate indexes | Yes | `header` checkbox and `columns=indexed`. |
| Preserve feature context and properties | Yes | `columns=feature/full` and `properties=true`. |
| Handle elevation/Z values | Yes | `elevation=auto|always|never`. |
| Drop repeated polygon closing vertices | Yes | `dedupe=ring-close`; stronger `adjacent` and `all` levels included. |
| Decimal rounding | Yes | `precision=-1..15`. |
| Delimiter choices for spreadsheet locales | Yes | `delimiter=comma|semicolon|tab|pipe`. |
| CRS reprojection / coordinate transformation | No | Out of model: requires projection database and UX beyond simple extraction. Documented as unsupported. |
| Multi-format GIS upload/download conversion | No | Out of model for this pure browser block; this repo tool accepts pasted GeoJSON text only. |
| Geometry repair/topology validation/simplification | No | Out of model; malformed GeoJSON returns clear errors instead of repair attempts. |

## Descriptor and page decisions

The descriptor uses enum controls for `order`, `columns`, `shapes`, `elevation`, `dedupe`, and `delimiter`, booleans for `properties`/`header`, and a bounded integer `precision`. Page examples cover line vertices with elevation, polygon ring-close removal, lat/lon spreadsheet output with properties, and full audit columns.

Copy is generic and brand-free. It emphasizes the key distinction from common GeoJSON-to-CSV tools: this one is coordinate-level, not feature-level.

# geojson-merge — competitor analysis (2026-07-31)

Scan before implementation. Goal: combine multiple GeoJSON files, FeatureCollections, Features, or bare geometries into one FeatureCollection. Notes paraphrased from public docs/tools; no competitor copy or branding reused.

## Competitors / references inspected

1. **Mapshaper merge / combine workflows** — accepts several vector layers/files, can merge layers and export GeoJSON; often paired with simplification/precision reduction.
2. **GDAL/OGR `ogrmerge.py` and `ogr2ogr` workflows** — command-line merge of geospatial datasets; preserves features, can assign source layer/file fields, exports GeoJSON.
3. **geojson-merge / geojson-flatten style npm CLIs** — concatenate FeatureCollections and Features into a single FeatureCollection; often supports line-delimited JSON pipelines.

## Table-stakes mapped to this tool

| Capability | In model? | Decision |
|---|---:|---|
| Accept FeatureCollections | yes | FeatureCollections contribute their `features` array. |
| Accept bare Features | yes | Bare Features are appended as-is. |
| Accept bare geometries | yes | Raw geometry objects are wrapped as Features with empty properties. |
| Output a single FeatureCollection | yes | Always emits RFC 7946 `FeatureCollection`. |
| Preserve feature order | yes | Features are concatenated in input order. |
| Pretty/minified output | yes | `indent` 0-8; 0 minifies. |
| Coordinate precision reduction | yes | `precision` -1..15, rounds every coordinate recursively. |
| Source tagging | yes | `source_property` stores 1-based input document number. |
| Avoid id collisions | yes | `renumber` assigns sequential numeric ids. |
| Topological dissolve/union | no | Out-of-model for a pure structural merge; would require geometry overlay algorithms. |
| Reprojection | no | GeoJSON is assumed WGS84; CRS conversion is out-of-scope. |
| Schema reconciliation/deduplication | no | Listed as a limit; this tool keeps all features. |

## UX decisions applied

- Multiline paste box with example chips for a basic merge, coordinate rounding/minify, and renumber/source tagging.
- Slider for `indent` because competitors expose pretty/minified export toggles and this repo supports numeric slider controls.
- Explicit limits in page copy: structural concatenation only, no dissolve/dedupe/reprojection.

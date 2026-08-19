## About this tool

GeoJSON is plain JSON, but a useful formatter needs to understand the geography inside it. This tool rewrites one GeoJSON document **in place**: a `FeatureCollection` stays a `FeatureCollection`, a single `Feature` stays a `Feature`, and a bare `Point`, `Polygon`, or `GeometryCollection` stays a bare geometry.

Use it to:

- pretty-print pasted GeoJSON for review, or set **Indent units** to `0` to minify it;
- round every coordinate to a fixed number of decimal places;
- preserve input key order, use canonical GeoJSON member order, or sort all keys alphabetically;
- add, recompute, or strip `bbox` members;
- rewind polygon rings to the RFC 7946 right-hand rule;
- keep or drop selected feature properties, including empty values;
- validate common RFC 7946 problems before output: missing `type`, bad geometry type, out-of-range coordinates, missing Feature members, and unclosed polygon rings.

All processing runs locally in your browser as WebAssembly. The output is still GeoJSON text, so you can copy it straight into a map library, commit it, or pipe it through another tool.

### Worked example — pretty-print and validate

Input:

```json
{"type":"Feature","properties":{"name":"Park","note":""},"geometry":{"type":"Point","coordinates":[12.3456789,-9.8765432]}}
```

With the defaults (`indent = 2`, validation on, full precision kept), output starts as:

```json
{
  "type": "Feature",
  "properties": {
    "name": "Park",
    "note": ""
  },
  "geometry": {
    "type": "Point",
    "coordinates": [
      12.3456789,
      -9.8765432
    ]
  }
}
```

### Worked example — minify and round coordinates

Set **Indent units** to `0` and **Coordinate decimal places** to `5` for URL/API payloads:

```json
{"type":"Point","coordinates":[12.34568,-9.87654]}
```

Approximate coordinate precision guidance:

| Decimal places | Rough ground precision near the equator |
| --- | --- |
| 3 | about 100 m |
| 4 | about 10 m |
| 5 | about 1 m |
| 6 | about 10 cm |

Rounding is lossy: keep a full-precision original when the geometry is authoritative.

### Worked example — canonical keys and bounding boxes

For a FeatureCollection with a line from `[0,1]` to `[4,-2]`, choose **Key order: Canonical GeoJSON order** and **Bounding boxes: Add feature bboxes + top-level bbox**. The output includes both:

```json
{"type":"FeatureCollection","bbox":[0.0,-2.0,4.0,1.0],"features":[{"type":"Feature","bbox":[0.0,-2.0,4.0,1.0],"geometry":{"type":"LineString","coordinates":[[0,1],[4,-2]]},"properties":{}}]}
```

Canonical order places GeoJSON members where people expect to find them (`type`, `id`, `bbox`, `coordinates` / `geometries`, `geometry`, `properties`, `features`) without sorting your `properties` object. Pick **Alphabetical** only when you want every object, including properties, sorted.

### Property pruning

`keep_properties` is a whitelist: if you enter `name,id`, all other feature properties are removed. `drop_properties` removes named properties after the keep-list is applied. **Drop empty/null feature properties** removes values that are `null`, `""`, `[]`, or `{}`.

This only touches `Feature.properties`; geometry coordinates and top-level metadata are left alone.

### Limits and edge cases

- The input must be a single GeoJSON object. For NDJSON, concatenated features, or many files, use `geojson-merge` first.
- Validation assumes RFC 7946 / WGS 84 coordinate order: `[longitude, latitude, altitude?]`. Projected coordinates or swapped `[lat, lon]` pairs may fail range checks; turn validation off only when you know the input is intentionally nonconforming.
- Bounding boxes are simple min/max boxes. A geometry crossing the antimeridian may need a hand-tuned RFC-style bbox rather than the naive global span.
- Low coordinate precision can collapse tiny rings, introduce self-intersections, or move boundaries enough to matter.
- The whole document is parsed in memory. Very large GeoJSON files belong in a desktop GIS or streaming command-line workflow.

## FAQ

<details>
<summary>What is the difference between pretty-printing and minifying?</summary>

Pretty-printing adds newlines and indentation so a human can review the structure in a diff or editor. Minifying (`indent = 0`) removes optional whitespace and emits one compact line, which is better for API payloads and files served over the network. Neither mode changes coordinates or properties unless you also enable rounding, key ordering, bbox, winding, or property-pruning options.

</details>

<details>
<summary>Should I use canonical or alphabetical key order?</summary>

Use **canonical** when you want GeoJSON objects to be easy to scan: `type` first, then `id`, `bbox`, geometry fields, `properties`, and `features`. It deliberately leaves the order inside `properties` alone because that object is your application data. Use **alphabetical** for deterministic generic JSON diffs where every object, including `properties`, should be sorted.

</details>

<details>
<summary>What does RFC 7946 polygon winding mean?</summary>

RFC 7946 recommends the right-hand rule: exterior polygon rings are counterclockwise and holes are clockwise. Many renderers tolerate either orientation, but some pipelines use winding to decide which side is filled. Choose **RFC 7946 right-hand rule** to reverse rings that are the wrong way around while keeping their coordinates otherwise unchanged.

</details>

<details>
<summary>Why did validation reject coordinates that look numeric?</summary>

GeoJSON positions are `[longitude, latitude]`, not `[latitude, longitude]`, and RFC 7946 uses WGS 84 ranges: longitude -180..180 and latitude -90..90. A coordinate like `[200, 10]` is numeric JSON, but it is not a valid WGS 84 longitude. If your data is in a projected CRS or deliberately outside RFC 7946, turn validation off to format it without conformance checks.

</details>

<details>
<summary>Does adding a bbox change my geometry?</summary>

No. A `bbox` is metadata derived from the coordinates: `[minLon, minLat, maxLon, maxLat]` for 2D data, or six numbers when every position has altitude. The coordinates are not clipped or simplified. Choose **Strip every bbox** if you want to remove stale bounding boxes before another system recomputes them.

</details>

<details>
<summary>Can this tool merge multiple GeoJSON files or GeoJSON Lines?</summary>

No; this tool formats one document while preserving its shape. Use `geojson-merge` for multiple features, many files, or line-delimited GeoJSON, then use this formatter on the merged output if you need canonical keys, coordinate rounding, bbox handling, winding, or property pruning.

</details>

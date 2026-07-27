## About this tool

GeoJSON is great for maps, but a spreadsheet or BI tool often wants one row per
feature. This converter flattens a GeoJSON `FeatureCollection`, a single `Feature`,
a bare geometry, or an array of those into CSV:

- every `properties` key becomes a column, using the union of keys across all
  features in first-seen order;
- missing property values are blank;
- geometry can be emitted as a 2-D WKT `geometry` column, `longitude`/`latitude`
  columns from the first coordinate, both, or omitted;
- nested properties can stay as compact JSON text or expand to dot-notated columns
  such as `address.city` and `tags.0`.

All conversion runs locally in your browser.

### Worked example

Input:

```json
{"type":"FeatureCollection","features":[
  {"type":"Feature","properties":{"name":"Alpha","pop":1200},"geometry":{"type":"Point","coordinates":[-105,40]}},
  {"type":"Feature","properties":{"name":"Beta","pop":800},"geometry":{"type":"Point","coordinates":[-106.5,41.25]}}
]}
```

With the default `geometry = wkt`, the output is:

```csv
name,pop,geometry
Alpha,1200,POINT (-105 40)
Beta,800,POINT (-106.5 41.25)
```

Set `geometry = lonlat` if your spreadsheet wants coordinate columns instead.

## FAQ

<details>
<summary>What GeoJSON inputs are accepted?</summary>

You can paste a `FeatureCollection`, a single `Feature`, a bare geometry such as a
`Point` or `Polygon`, or a top-level array of features/geometries. A bare geometry
has no properties, so the CSV contains only geometry columns unless you choose
`geometry = none` (which would leave nothing to output).

</details>

<details>
<summary>How are geometry coordinates represented?</summary>

`geometry = wkt` writes one `geometry` column as Well-Known Text, which works for
points, lines, polygons, multi-geometries, and geometry collections. `geometry =
lonlat` writes the first coordinate as `longitude` and `latitude`; for lines and
polygons this is the first vertex. `both` emits both representations.

</details>

<details>
<summary>What happens to nested properties?</summary>

By default, nested objects and arrays stay in a single cell as compact JSON so no
information is lost. Choose `nested = flatten` to expand them into dot-notated leaf
columns, for example `address.city` or `tags.0`.

</details>

<details>
<summary>Does this reproject coordinates?</summary>

No. GeoJSON coordinates are used exactly as provided, normally `[longitude,
latitude]` in WGS84. Reprojection, simplification, topology repair, and format
exports such as Shapefile/GeoPackage are GIS processing tasks outside this small
CSV flattener.

</details>

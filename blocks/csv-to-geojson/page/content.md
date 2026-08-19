## About this tool

Use this CSV to GeoJSON converter when you have a spreadsheet export, database query, GPS sample,
or JSON row list with latitude and longitude columns and need map-ready RFC 7946 GeoJSON. Paste a
CSV, TSV, semicolon, pipe-delimited table, or a JSON array of objects. The converter auto-detects
common coordinate headers such as `lat`, `latitude`, `lon`, `lng`, `longitude`, `x`, and `y`, or you
can name the coordinate columns explicitly.

By default the result is a GeoJSON `FeatureCollection` of Point features. Non-coordinate columns are
kept as feature properties, with conservative type inference for booleans, numbers, blanks, and
strings. You can also join the rows in order into one `LineString` route or one closed `Polygon`
ring, add a `bbox`, round coordinates, preserve bad rows as `null` geometries, or stop on the first
invalid coordinate.

### Worked example

Paste this CSV, set `precision = 4`, and leave `shape = points`:

```csv
name,lat,lon
Denver,39.7392,-104.9903
Boulder,40.0150,-105.2705
```

The output is a GeoJSON FeatureCollection with two Point features. Coordinates are emitted in
GeoJSON order (`[longitude, latitude]`), while `name` remains in `properties`. Switch `shape` to
`line` to turn the same row order into a single LineString route, or set `bbox = true` to include the
coordinate bounds.

### Limits and edge cases

- Input must be a delimited table with a header row, a JSON array of objects, or a wrapper object
  containing an array of objects.
- Up to 100,000 data rows are accepted per run.
- Latitude must be between -90 and 90; longitude must be between -180 and 180.
- GeoJSON coordinates are always `[longitude, latitude]`, with optional elevation as the third
  ordinate.
- `shape = line` needs at least two valid coordinate rows; `shape = polygon` needs at least three.
- Polygon rings are closed automatically and normalized to counterclockwise exterior-ring winding.
- Semicolon files with comma-decimal coordinates such as `59,91;10,75` are supported.
- Type inference is conservative: values like `00501` remain strings so identifiers are not damaged.

## FAQ

<details>
<summary>Why are coordinates output as longitude, latitude?</summary>

GeoJSON follows RFC 7946 coordinate order, which is `[longitude, latitude]`. Many CSV files label
columns as `lat,lon` because people usually say latitude first, so this tool deliberately swaps them
when building GeoJSON. The original coordinate columns are removed from properties to avoid
representing the same values twice.

</details>

<details>
<summary>Can I convert JSON rows instead of CSV?</summary>

Yes. Paste an array of objects such as `[{
"name":"A","lat":40,"lon":-105}]`, or a wrapper object that contains an array of row objects. The
converter preserves the union of object keys as columns, auto-detects the coordinate fields, and
writes every other key into `properties`.

</details>

<details>
<summary>What happens to rows with blank or invalid coordinates?</summary>

The `invalid` setting controls that. `skip` omits invalid coordinate rows, which is useful for messy
exports. `error` stops at the first bad row and tells you which coordinate failed. `null` keeps the
row's properties but writes `geometry: null`, which is useful when downstream QA needs to see which
records had missing coordinates.

</details>

<details>
<summary>How do I make a route or polygon?</summary>

Set `shape` to `line` for a LineString route or `polygon` for a Polygon. Rows are used in their
current order, so sort the table before pasting if route order matters. Lines need at least two valid
points. Polygons need at least three valid points; the ring is closed automatically and normalized to
GeoJSON exterior-ring winding.

</details>

<details>
<summary>Does this geocode addresses or reproject coordinates?</summary>

No. This tool converts existing WGS84 latitude/longitude values into GeoJSON. It does not look up
addresses, call a map API, transform UTM/state-plane coordinates, or change datums. If your data is
not already latitude/longitude in decimal degrees, convert it first before using this page.

</details>

## About this tool

`geojson-wkt` converts simple-feature geometries between GeoJSON, WKT/EWKT, and WKB/EWKB. It is meant for GIS debugging, PostGIS copy/paste workflows, API payload cleanup, and format handoffs where you need the geometry itself rather than a full map or database.

The converter accepts GeoJSON `Geometry`, `Feature`, `FeatureCollection`, `GeometryCollection`, or arrays of those. WKT input may include an `SRID=4326;` EWKT prefix. WKB input may be hex or base64 text, one geometry per line. Output can be a single geometry or one geometry per line when a FeatureCollection contains several features.

### Worked example

Input GeoJSON:

```json
{"type":"Point","coordinates":[30,10]}
```

Default output:

```text
POINT(30 10)
```

With `to=geojson`, `input=SRID=4326;POINT Z (30 10 5)`, and pretty output enabled, the result is:

```json
{
  "type": "Point",
  "coordinates": [
    30,
    10,
    5
  ]
}
```

### Options and limits

- **Input format** can be auto-detected or forced to GeoJSON, WKT/EWKT, or WKB/EWKB.
- **Output format** can be WKT/EWKT, GeoJSON, or WKB/EWKB.
- **Multiple geometries** either wrap into a `GEOMETRYCOLLECTION` or emit one converted geometry per line.
- **SRID to write** adds EWKT/EWKB metadata. Coordinates are not reprojected.
- **Coordinate precision** rounds coordinates from 0 to 15 decimals; `-1` keeps full shortest-round-trip formatting.
- **WKB text encoding** chooses hex or base64; **WKB byte order** chooses little-endian/NDR or big-endian/XDR.
- The input limit is 2 MB, and nested GeometryCollections are limited to 32 levels.
- Supported geometry types are Point, LineString, Polygon, MultiPoint, MultiLineString, MultiPolygon, and GeometryCollection, including `EMPTY`, 2D, Z, M, and ZM forms.
- Curved or surface extensions such as `CIRCULARSTRING`, `TIN`, and `TRIANGLE` are rejected with a clear error instead of being approximated.

## FAQ

<details>
<summary>Does this reproject coordinates between coordinate systems?</summary>

No. An SRID is metadata only here. `SRID=4326;POINT(30 10)` stays at the same numeric coordinates; the tool does not apply EPSG transformations or use a projection database.

</details>

<details>
<summary>What happens to GeoJSON feature properties?</summary>

WKT and WKB are geometry-only formats, so feature properties are not represented in those outputs. A GeoJSON FeatureCollection is converted from its `geometry` members, and features with `geometry: null` are skipped.

</details>

<details>
<summary>Can it read and write WKB?</summary>

Yes. Paste hex WKB/EWKB or base64 WKB/EWKB as text. For output, choose `to=wkb`, then choose hex or base64 and little-endian or big-endian byte order.

</details>

<details>
<summary>Why do M values disappear in GeoJSON output?</summary>

GeoJSON positions are longitude/latitude plus optional elevation. There is no standard M ordinate, so WKT/WKB `M` values are dropped when producing GeoJSON. Z values are kept as the third coordinate.

</details>

<details>
<summary>Is the input uploaded?</summary>

No. The Rust converter runs locally in WebAssembly on the page and locally in the CLI. It does not fetch map tiles, call a geocoder, or send your geometry anywhere.

</details>

## About this tool

GeoJSON Coordinate Extractor turns geometry positions into spreadsheet-friendly rows. Paste a FeatureCollection, a single Feature, a bare geometry, or an array of GeoJSON objects and the tool emits one CSV row per coordinate in document order. That is different from a feature-level GeoJSON-to-CSV converter: a LineString with 20 vertices becomes 20 rows, and a polygon ring can include every corner plus the closing vertex unless you choose to drop it.

Use the controls to switch between GeoJSON order (`longitude,latitude`) and map-friendly order (`latitude,longitude`), include index and feature context columns, keep only points/lines/polygons, round coordinates, add elevation when present, append feature properties, or remove repeated coordinates. Processing runs locally in the browser and accepts documents up to 16 MiB or 200,000 emitted coordinate rows.

### Worked example

Input GeoJSON:

```json
{"type":"Feature","properties":{"name":"Ridge trail"},"geometry":{"type":"LineString","coordinates":[[-105.1,40.1,2410],[-105.2,40.2,2530]]}}
```

With the default options, the output is:

```csv
longitude,latitude,elevation
-105.1,40.1,2410
-105.2,40.2,2530
```

Choose `columns=full` when you need audit fields such as feature index, geometry type, part, ring, position, and whether a polygon coordinate is the repeated closing vertex.

## FAQ

<details>
<summary>Does this convert GeoJSON features to one CSV row per feature?</summary>

No. This tool extracts coordinates, so it writes one row per GeoJSON position. For example, a 500-vertex polygon produces 500 coordinate rows unless you filter or dedupe it. Use a feature-level GeoJSON-to-CSV converter when you need one row per Feature.

</details>

<details>
<summary>Why are longitude and latitude in that order?</summary>

GeoJSON stores coordinates as `[longitude, latitude]`, so the default CSV columns are `longitude,latitude`. If your spreadsheet, GPS workflow, or map UI expects `latitude,longitude`, switch Coordinate order to `latlon`; the header and values are both swapped.

</details>

<details>
<summary>What happens to polygon closing coordinates?</summary>

GeoJSON polygon rings often repeat the first coordinate as the last coordinate. The default keeps that repeated closing vertex because it is present in the source. Set Repeated coordinates to `ring-close` to drop those closing repeats, or use `full` columns to see a `ring_close` flag for the repeated vertex.

</details>

<details>
<summary>Can it keep Feature properties with each coordinate?</summary>

Yes. Enable Append feature property columns to add the union of property keys as extra CSV columns, repeated on every coordinate row emitted from that feature. Nested property objects and arrays are written as compact JSON strings.

</details>

<details>
<summary>What are the limits and unsupported cases?</summary>

The tool accepts valid GeoJSON geometry types: Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, GeometryCollection, Feature, FeatureCollection, or a top-level array of those. It does not reproject coordinates, validate topology, simplify geometry, fetch URLs, or repair malformed GeoJSON. Inputs over 16 MiB or outputs over 200,000 coordinate rows should be split first.

</details>

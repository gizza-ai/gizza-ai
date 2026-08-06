## About this tool

**TopoJSON to GeoJSON Converter** expands a compact TopoJSON topology back into the standard
GeoJSON that mapping libraries, GIS desktops and spatial databases actually read. Leaflet,
Turf.js, PostGIS, QGIS and `geopandas` all consume GeoJSON; TopoJSON is a distribution format,
and this is the step that turns one into the other.

TopoJSON is small because it never stores a boundary twice. Every line segment lives once in a
shared **arc** list, and each shape refers to arcs by index — a negative index like `-1` means
"arc 0, walked backwards". On top of that, a quantized topology stores integer grid coordinates
plus a `transform` (a `scale` and a `translate`), and each arc's positions are **delta-encoded**:
the first position is absolute and every later one is an offset from the position before it.

Expanding all three layers is what this tool does: decode each arc once, accumulate the deltas,
apply the transform, then stitch the referenced arcs back together — dropping the duplicated
position where two arcs meet, and reversing the ones referenced negatively — until every ring
closes.

### Worked example

Two unit squares that share their middle edge. The shared edge is stored once, as arc `0`; the
east square reuses it backwards as `-1`:

```json
{
  "type": "Topology",
  "transform": { "scale": [0.5, 0.5], "translate": [10, 20] },
  "objects": {
    "blocks": {
      "type": "GeometryCollection",
      "geometries": [
        { "type": "Polygon", "id": "a", "properties": { "name": "West" }, "arcs": [[0, 1]] },
        { "type": "Polygon", "id": "b", "properties": { "name": "East" }, "arcs": [[2, -1]] }
      ]
    }
  },
  "arcs": [
    [[2, 0], [0, 2]],
    [[2, 2], [-2, 0], [0, -2], [2, 0]],
    [[2, 0], [2, 0], [0, 2], [-2, 0]]
  ]
}
```

With **Indent = 0** (minified), the output is:

```json
{"type":"FeatureCollection","features":[{"type":"Feature","id":"a","properties":{"name":"West"},"geometry":{"type":"Polygon","coordinates":[[[11.0,20.0],[11.0,21.0],[10.0,21.0],[10.0,20.0],[11.0,20.0]]]}},{"type":"Feature","id":"b","properties":{"name":"East"},"geometry":{"type":"Polygon","coordinates":[[[11.0,20.0],[12.0,20.0],[12.0,21.0],[11.0,21.0],[11.0,20.0]]]}}]}
```

Both rings are now fully written out and closed, the quantized `[2, 0]` grid values have become
real coordinates `[11.0, 20.0]` via the transform, and `id` and `properties` came through
untouched. Leave **Indent** at its default of `2` to get the same thing pretty-printed.

### Options

- **Object to expand** — the name of one entry in the topology's `objects` map, e.g. `countries`.
  Leave it blank (the default) to expand every object and merge them into a single collection, in
  document order. A name that isn't there is rejected with the list of names the file does have.
- **Output shape** — *FeatureCollection* (default) is standard RFC 7946: each geometry becomes a
  `Feature` that keeps its `properties`, `id` and `bbox`. *GeometryCollection* emits the bare
  geometries instead, which is what you want when the consumer only cares about shapes.
- **Add a bounding box** — appends `"bbox": [west, south, east, north]` to the result. It's
  computed from the coordinates actually emitted, so selecting a single object gives that object's
  extent rather than the whole topology's.
- **Coordinate decimals** — rounds every coordinate to N places. Quantized grids decode to values
  like `-179.99999999999997`; rounding to 6 or 7 decimals removes that floating-point noise (7
  decimals is roughly 1 cm of longitude) and shrinks the file. `-1`, the default, keeps full
  precision.
- **Indent spaces** — 1–8 spaces per level (default 2), or `0` to minify onto one line.

### Limits and behavior

- **The output is bigger than the input — that's the point.** GeoJSON has no topology, so every
  shared border gets written out once per shape that touches it. Expect roughly 2–5× the byte
  count, more for dense administrative boundaries.
- Un-quantized topologies (no `transform` member) are handled too: their arc positions are read as
  absolute coordinates rather than deltas.
- `Point` and `MultiPoint` coordinates are quantized but **not** delta-encoded, per the TopoJSON
  spec, so the transform is applied to each position on its own.
- A third dimension (elevation) or any further ordinate is carried through unchanged — the spec
  only quantizes x and y.
- The computed `bbox` is 2D, `[west, south, east, north]`, even for 3D coordinates. A `bbox`
  already present on an individual geometry is copied to its feature as-is.
- Degenerate input is padded to stay valid GeoJSON: a ring stitched from a two-position arc is
  padded to the 4 positions RFC 7946 requires.
- A geometry whose `"type"` is `null` becomes a feature with a `null` geometry, which GeoJSON
  allows. An unrecognized type name is an error, not a silent drop.
- Everything runs locally in WebAssembly, so file size is bounded only by your browser's memory.
  Multi-megabyte topologies work but take a moment to render.

## FAQ

<details>
<summary>What are TopoJSON "arcs", and why does my file look like nonsense?</summary>

An arc is a shared line segment stored exactly once. Instead of repeating the border between two
countries in both countries' coordinate lists, TopoJSON stores it as one arc and has both shapes
point at its index. That's why the raw file is full of small integers rather than latitudes and
longitudes — the numbers are grid positions, and mostly *offsets* from the previous position.
Expanding the arcs is exactly what this converter does.

</details>

<details>
<summary>What does a negative arc index like -1 mean?</summary>

It means the arc is traversed backwards. TopoJSON encodes the reversal as the bitwise complement
of the index, so `-1` is arc `0` reversed, `-2` is arc `1` reversed, and so on. A shared border is
walked one way by the shape on its left and the other way by the shape on its right, which is how
both rings end up closed while the coordinates are only stored once.

</details>

<details>
<summary>My TopoJSON has several objects. Which one do I get?</summary>

All of them, merged into one collection, unless you name one. Put a name in **Object to expand**
— it's the key in the file's `objects` map, often something like `countries`, `states` or
`counties` — to convert just that layer. Typing a name that isn't in the file gives you an error
listing the names that are, so you never have to guess twice.

</details>

<details>
<summary>Why are my coordinates full of digits like 179.99999999999997?</summary>

That's binary floating-point noise from applying the quantization transform, not corrupted data —
the true value is 180. Set **Coordinate decimals** to 6 or 7 and it rounds away, which also makes
the output noticeably smaller. Seven decimal places is about a centimetre of longitude, far
finer than any quantized topology's real accuracy.

</details>

<details>
<summary>Are properties and feature ids preserved?</summary>

Yes. Each geometry's `properties` object is copied over verbatim, with its key order intact, and
an `id` is emitted on the feature when the source has one. A geometry with no `properties` gets an
empty `{}` so the output is always valid RFC 7946. Choosing the *GeometryCollection* output shape
is the one case where they're dropped — that mode emits geometry only, by design.

</details>

<details>
<summary>Can it convert GeoJSON back into TopoJSON?</summary>

No — this converter only goes TopoJSON → GeoJSON. Going the other way means detecting shared
borders, cutting them into arcs and choosing a quantization grid, which is a different algorithm
rather than a reverse switch. If you paste GeoJSON here you'll get an error saying so instead of
a confusing failure.

</details>

<details>
<summary>Why is the GeoJSON so much larger than the TopoJSON I started with?</summary>

Because the topology is gone. TopoJSON's size advantage comes entirely from storing each shared
border once; GeoJSON has no way to express sharing, so every boundary is repeated for each shape
that uses it. Two to five times the size is normal. Minifying (**Indent = 0**) and rounding
(**Coordinate decimals**) claw a useful amount of that back.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The conversion runs in your browser via WebAssembly — the file never leaves your device, and
there's no account or upload step. That also means it keeps working offline once the page has
loaded.

</details>

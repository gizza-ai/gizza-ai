## About this tool

**GeoJSON Merge** combines two or more GeoJSON inputs into a single
[RFC 7946](https://datatracker.ietf.org/doc/html/rfc7946) **FeatureCollection**.
Paste your GeoJSON one value after another — separated by whitespace, a blank
line, or one object per line (line-delimited GeoJSON / NDJSON works too) — and
every feature is flattened, **in order**, into one collection.

Mixed input shapes are handled for you:

- A **FeatureCollection** contributes all of its `features`.
- A bare **Feature** is kept as-is.
- A raw **geometry** (`Point`, `LineString`, `Polygon`, `GeometryCollection`, …)
  is wrapped into a `Feature` with empty `properties`.

### Options

- **Indent** — spaces per level for the output; set `0` to minify to a single line.
- **Coordinate precision** — round every coordinate to N decimal places to shrink
  the file (e.g. `6` ≈ ~0.1 m). Leave it at `-1` to keep full precision.
- **Renumber feature ids** — overwrite each feature's `id` with a fresh
  sequential integer (`0`, `1`, `2`, …), fixing id collisions across sources.
- **Tag features with source property** — name a property and each feature gets it
  set to its **1-based input-document number**, so merged features stay traceable
  to the file they came from.

Everything runs **locally in your browser** via WebAssembly — your data is never
uploaded.

### Worked example

Merging a FeatureCollection with a bare point:

```
{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A"},"geometry":{"type":"Point","coordinates":[0,0]}}]}
{"type":"Point","coordinates":[1,1]}
```

produces (minified):

```
{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A"},"geometry":{"type":"Point","coordinates":[0,0]}},{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[1,1]}}]}
```

### Good for

- Reassembling tiled or regional exports (per-county, per-day) into one dataset.
- Consolidating field-collected features from several files before upload.
- Cleaning up an oversized GeoJSON by trimming coordinate precision on the way in.

### Limits

- Pure **concatenation** — it does not deduplicate features, reconcile differing
  property schemas, or dissolve/union overlapping geometries.
- No coordinate reprojection: GeoJSON is assumed to be WGS84 (EPSG:4326).
- Inputs must be valid JSON; a parse error reports which value (`#N`) failed.

## FAQ

<details>
<summary>What input formats can I paste in?</summary>

Any mix of GeoJSON **FeatureCollections**, bare **Features**, and raw
**geometries** — concatenated with whitespace, blank lines, or one object per
line (line-delimited GeoJSON / NDJSON). They're read as a stream of JSON values
and merged left to right; a single value is simply reformatted.

</details>

<details>
<summary>Does it deduplicate or dissolve overlapping features?</summary>

No. This is a structural merge: every feature from every input is kept, in
order. It does not remove duplicates, union geometries, or dissolve shared
borders — use a full GIS tool for topology operations.

</details>

<details>
<summary>How does coordinate precision rounding work?</summary>

Set **Coordinate precision** to the number of decimal places you want and every
coordinate in every geometry is rounded to that many decimals — a quick way to
shrink a file (6 decimals is roughly 0.1 m at the equator). Leave it at `-1` to
keep coordinates exactly as they came in.

</details>

<details>
<summary>Can I keep track of which file each feature came from?</summary>

Yes — type a property name into **Tag features with source property** and every
feature gets that property set to its 1-based source-document number (the first
pasted value is `1`, the second `2`, and so on), so you can style or filter by
origin after merging.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The merge runs entirely in your browser through WebAssembly — nothing is
sent to a server, so it works offline and keeps private data private.

</details>

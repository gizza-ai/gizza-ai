# geojson-format — competitor analysis (2026-08-13)

Scan run **before** implementation, per `create-tool-loop` step 4. All competitor notes are
**paraphrased observations of behaviour/options** — no competitor copy, branding, or trademarks
are reproduced or reused. Out-of-model items are listed, not built.

Backlog row: `geojson-format` — "Pretty-print, minify, or re-key GeoJSON and round coordinate
precision." (type hint: pure)

## Duplicate check (done first)

`ls blocks/ | grep -iE 'geo|json'` surfaced the neighbours below. **Not a duplicate:**

| Existing block | Why it does not cover this row |
| --- | --- |
| `json-beautify` | Generic JSON pretty/minify + validate. Geo-unaware: no coordinate-precision rounding, no RFC 7946 member ordering, no bbox, no ring/winding handling, no property pruning. |
| `geojson-merge` | Combines **multiple** inputs into one FeatureCollection; it always rewrites the document into a `FeatureCollection` and only exposes indent/precision/renumber/source-tag. It cannot format a single document **in place** (a bare `Feature`/geometry stays a bare `Feature`/geometry), and has no key ordering, bbox, winding, or property pruning. |
| `geojson-to-csv`, `geojson-to-svg`, `topojson-to-geojson`, `shapefile-to-geojson` | Format **converters** (GeoJSON → other / other → GeoJSON), not a GeoJSON→GeoJSON formatter. |
| `json-sort` | Sorts JSON object keys/arrays generically; no GeoJSON canonical member order, no geometry awareness. |
| `geofence-check`, `geo-cluster`, `gpx-*` | Geometry analysis / GPS track tooling, unrelated. |

Skiplist grep for `geojson` returned no line pointing at this slug.

## Competitors reviewed

One WebSearch (`online GeoJSON formatter pretty print minify coordinate precision tool`),
then the top reachable tools were read directly.

### C1 — geoutil.com "GeoJSON minifier"
- Coordinate-precision control in decimal places, presented with real-world accuracy hints
  (5 dp ≈ ~1 m at the equator); precision is the headline control, default around 5.
- Selective property retention/removal with "quick presets" for common pruning cases.
- Whitespace removal (minify); optional removal of empty arrays/objects.
- Auto-detects NDJSON / GeoJSON-Lines and folds it into a FeatureCollection.
- Preview before download; claims client-side only, ~100 MB-class files.

### C2 — geojsonkit.org "GeoJSON formatter" + "GeoJSON validator"
- Pretty-print with a chosen indent, or minify to a single line.
- Separate validator: syntax errors plus RFC 7946 conformance, reported with the location —
  explicitly covering geometry types, coordinate ranges, and polygon ring closure.
- Stated principle: properties are preserved exactly, and coordinate precision is never changed
  unless the user asks.
- Browser-local, no stated size limit.

### C3 — open-innovations.github.io/geojson-minify
- Single control: coordinate precision in decimal places, with the educational framing that
  5 dp ≈ 1 m and 4 dp ≈ 10 m resolution.
- Strips whitespace; outputs a downloadable minified file. Assumes RFC 7946 / WGS 84.
- No property pruning or other options.

### C4 — formatjsononline.com "GeoJSON formatter"
- Format vs minify mode; indentation choices of 2, 4, 8 spaces **or tabs**.
- **Alphabetical key sorting.**
- Automatic bounding-box calculation; coordinate validation.
- Validation covers JSON syntax, RFC 7946 conformance, geometry types, coordinate arrays,
  feature properties, and warns about a CRS member.
- Reports statistics: feature count, geometry-type breakdown, bbox, file size.
- Sample documents (Point / LineString / Polygon) preloaded as one-click examples;
  FAQ covers what GeoJSON is, validation, the seven geometry types, large files, privacy,
  format-vs-minify, and what a bounding box is.

(geojason.com's pretty-printer 404'd at fetch time; C4 replaced it so the scan still covers
four real tools, as required.)

## Table stakes → decision

| Capability | Seen at | Decision |
| --- | --- | --- |
| Pretty-print with chosen indent | C2, C4 | **In-model** → `indent` (0–8, slider), default 2 |
| Minify to one line | C1, C2, C3, C4 | **In-model** → `indent = 0` |
| Tab indentation | C4 | **In-model** → `indent_char = space \| tab` |
| Coordinate precision rounding | C1, C2, C3 | **In-model** → `precision` (−1 = keep full, 0–15), slider; page copy carries the dp→metres table |
| Alphabetical key sorting | C4 | **In-model** → `key_order = alpha` |
| Canonical GeoJSON member order ("re-key", the backlog row's own wording) | implied by C2's RFC framing | **In-model** → `key_order = canonical` (`type`, `id`, `bbox`, `coordinates`/`geometries`, `geometry`, `properties`, `features`) |
| Bounding-box calculation | C4 | **In-model** → `bbox = keep \| add \| features \| strip`, 2D or 3D per RFC 7946 |
| Property pruning (keep-list / drop-list) | C1 | **In-model** → `keep_properties`, `drop_properties` (tag-list pills) |
| Drop empty/null property values | C1 | **In-model** → `drop_empty_properties` |
| RFC 7946 validation with located errors (geometry type, coordinate range, ring closure) | C2, C4 | **In-model** → `validate` (default on); every error names the JSON path and what was expected |
| Polygon winding / right-hand rule | RFC 7946 §3.1.6; not exposed by any scanned tool, but required for correct rendering in several map libraries | **In-model** → `winding = keep \| rfc7946` (a genuine differentiator, not a copy) |
| Preset examples | C4 | **In-model** → four `[[example]]` chips on the page |
| Statistics panel (feature counts, geometry mix, size savings) | C1, C4 | **Considered, rejected.** The tool's single output must stay a valid GeoJSON document that can be piped/downloaded/re-fed; interleaving a stats report would break that contract. `json-structure-analyzer` and `geojson-to-csv` cover counting/inspection. |
| NDJSON / GeoJSON-Lines input folding | C1 | **Considered, rejected — already shipped.** `geojson-merge` accepts concatenated / line-delimited GeoJSON and flattens it into one FeatureCollection; duplicating it here would blur two tools. Cross-linked from the page copy. |
| File upload / batch of many files, download button | C1, C4 | **Partly out-of-model.** gizza text pages take pasted text and already provide Copy + a Download link for `format = "text"`; multi-file batch needs a server-side or file-system surface. The CLI covers files (`gizza tool geojson-format geojson="$(cat x.geojson)"`). |
| Map preview of the result | C1 (preview), C4 | **Out-of-model** for this page (no map canvas in the generic tool page). `geojson-to-svg` renders GeoJSON visually. |
| 100 MB-class files | C1 | **Out-of-model at that scale.** Whole document is parsed in memory in wasm; the page states the practical guidance instead of pretending. |

## Feasibility spike notes

- `serde_json` with `preserve_order` gives full control over member order (needed for both
  `canonical` and `alpha`) and is already proven wasm-safe in `geojson-merge`.
- `PrettyFormatter::with_indent` takes an arbitrary byte slice, so tab indentation is free.
- Winding correction is a shoelace signed-area test per ring — pure arithmetic, no crate needed.
- 3D bbox (6 numbers) is emitted only when **every** position carries an altitude, matching
  RFC 7946; otherwise 2D (4 numbers).

## Known limits (stated on the page, not just in code)

- Bounding boxes are computed naively as min/max, so a document that crosses the antimeridian
  gets a bbox spanning the globe rather than the RFC's wrap-around form.
- Rounding coordinates is lossy and can self-intersect very dense polygons at low precision.
- The whole document is held in memory; multi-hundred-MB files belong in a desktop GIS.

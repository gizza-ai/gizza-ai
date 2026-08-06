# topojson-to-geojson — competitor analysis (2026-08-06)

Scan run **before** implementation, so the descriptor could ship the table-stakes from day one.
All findings are **paraphrased** — no competitor copy, branding, or trademarks are reproduced or
reused anywhere in this tool.

## Competitors reviewed

| # | Tool | Model | Notes |
|---|------|-------|-------|
| 1 | GeoDataTools — TopoJSON to GeoJSON | browser-local | Explicitly advertises transform-matrix (scale + translate) handling and delta-encoded arc decompression; drag-drop **or** pasted text; object picker when the topology holds several named objects; download. |
| 2 | GeoUtil — TopoJSON to GeoJSON | browser-local | Emits a single RFC 7946 `FeatureCollection`; **merges** all topology objects into that one collection; preserves feature properties; expands shared arcs to full coordinates; map preview + download. FAQ covers output-size growth and multi-object handling. |
| 3 | QuickMapTools — TopoJSON to GeoJSON | browser-local | File upload only (`.topojson`/`.json`); converts the **first** object by default; download; notes that the conversion discards the topology, so output grows. Batch conversion is a paid tier. |
| 4 | MyGeodata Cloud converter | **server** upload | Generic GIS format matrix, 5 GB uploads, results returned as a ZIP. Server-side by definition. |
| 5 | GroupDocs conversion app | **server** upload | Generic document/format converter with a TopoJSON→GeoJSON entry; upload-then-download. |

Only 1–3 are true peers for a browser-local, no-account tool; 4–5 are server converters and are
listed for completeness rather than as a model to match.

## Table stakes → where each landed

| Table stake | Seen in | Decision |
|---|---|---|
| Apply the `transform` matrix (`scale` + `translate`) | 1, 2 | **Built** — core `decode`, applied to arcs *and* to `Point`/`MultiPoint` coordinates. |
| Delta-decode arc positions | 1, 2 | **Built** — accumulate per arc, first position absolute. Arcs without a `transform` are read as absolute positions. |
| Stitch shared arcs, including reversed (`~i`) references | 1, 2 | **Built** — shared-endpoint de-duplication plus tail-reversal for negative indices. |
| Every geometry type (Point, MultiPoint, LineString, MultiLineString, Polygon, MultiPolygon, GeometryCollection) | 1, 2 | **Built** — all seven, plus an explicit `null`-type geometry. |
| Preserve `properties` | 1, 2, 3 | **Built** — copied verbatim, key order preserved. |
| Preserve feature `id` | 1 | **Built** — emitted when present (string or number). |
| Multiple named objects | 1 (picker), 2 (merge all), 3 (first only) | **Built** — the `object` param does both: name one object, or leave it blank to merge every object into one `FeatureCollection`. Superset of all three behaviors. |
| Output as one RFC 7946 `FeatureCollection` | 2 | **Built** — the default `output` mode. |
| Download the result | 1, 2, 3 | **Already platform** — text-format pages get a Download link plus Copy/Reset from the shared generator. |
| Browser-local, nothing uploaded | 1, 2, 3 | **Already** — wasm, no network. |
| State that output grows vs TopoJSON | 2, 3 | **Built as copy** — page explains the 2–5× growth and why (topology is discarded). |

## Gaps we close that no scanned competitor exposes

- **`bbox`** — optional RFC 7946 bounding box computed from the coordinates actually emitted, so it
  stays correct when only one object was selected. Per-geometry `bbox` values already in the
  TopoJSON are carried onto the matching `Feature`.
- **`precision`** — round coordinates to N decimals. Quantized topologies decode to values like
  `-179.99999999999997`; rounding removes that float noise and shrinks the output further.
- **`indent`** — pretty-print depth, or `0` to minify. Competitors preview pretty and download
  pretty with no control.
- **`output = geometry-collection`** — a bare GeoJSON `GeometryCollection` for consumers that want
  geometry without the `Feature` wrapper.
- **Named errors** — out-of-range arc index, unknown geometry type, unknown object name (with the
  available names listed), and non-Topology input each produce a specific message. The scanned
  tools surface generic failures.

## Considered, not built

- **Map preview** (GeoUtil, GeoDataTools) — *out of model.* Needs a tile/map renderer; this repo
  renders generic, dependency-free pages. `geojson-to-svg` covers visual inspection instead.
- **File upload / drag-drop of `.topojson`** — *out of model for a pure tool.* Pure blocks take
  text params; the page's paste field is the supported input path. Media/file inputs are an
  ffmpeg-runtime feature.
- **Batch / multi-file conversion** (QuickMapTools Pro, MyGeodata ZIP) — *out of model.* One
  document per run; there is no server-side job queue here.
- **GeoJSON → TopoJSON (reverse)** — *considered, rejected for this tool.* Quantization and
  topology-building are a materially different algorithm; it belongs in its own tool rather than as
  a direction flag that doubles this schema.
- **`topojson.merge` (dissolve shared borders into one polygon)** — *considered, rejected.* A real
  topojson-client feature, but it is a distinct spatial operation rather than a conversion option,
  and no scanned converter exposes it.
- **5 GB inputs** (MyGeodata) — *out of model.* Browser memory is the ceiling; the page states it.

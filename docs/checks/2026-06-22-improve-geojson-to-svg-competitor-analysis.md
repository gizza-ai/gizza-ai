# geojson-to-svg — competitor analysis (2026-06-22)

Tool: render GeoJSON features into a standalone SVG map, no tile servers, no network.
Pure-Rust (serde_json + a hand-rolled Web-Mercator projection); runs on all surfaces
(chat block, CLI, page).

## Surfaces verified

- **Chat/LLM block** — `wafer build` validates `target/block.wasm` instantiates (348.5 KiB). Schema
  single-sourced from `descriptor()`; drift-guard unit test passes.
- **CLI** — `gizza tool geojson-to-svg geojson='{…FeatureCollection…}'` returns valid `<svg>…</svg>`
  with a `<path fill-rule="evenodd">` polygon + a `<circle>` point.
- **Page** — `/tools/geojson-to-svg/`; 3 Playwright tests pass (polygon FeatureCollection → SVG,
  bare Point → circle, custom fill colour honoured).

## Competitors surveyed

1. **GeoUtil — GeoJSON to SVG** (geoutil.com) — 100% client-side, drag-and-drop upload, preview +
   download, "clean, optimized SVG markup".
2. **MyGeodata Cloud** (mygeodata.cloud) — server-side upload/convert to SVG and many GIS/CAD formats;
   batch; bounding-box auto-calc.
3. **Aspose GeoJSON Viewer / to SVG** (products.aspose.app) — online viewer, export to PNG/JPEG/BMP/PDF/SVG,
   no registration/watermark.
4. **GroupDocs GEOJSON→SVG / SVGZ** (products.groupdocs.app) — free online file conversion incl. SVGZ.
5. **geojson2svg** (github.com/gagan-bansal/geojson2svg) — JS library: GeoJSON→SVG string given a
   viewport size and map extent; client or Node.

## Feature diff (✓ = in our tool, ✗ = not / out of model)

| Capability                                              | Ours | Competitors |
|---------------------------------------------------------|------|-------------|
| All geometry types (Point/Multi*, Line, Polygon, GC)    | ✓    | ✓ (most)    |
| FeatureCollection / Feature / bare geometry roots       | ✓    | ✓           |
| Null geometry handled (skipped, not crash)              | ✓    | partial     |
| Automatic bounding box → fit viewport, aspect preserved | ✓    | ✓           |
| Polygon holes (inner rings) cut out (evenodd)           | ✓    | ✓ (varies)  |
| Web-Mercator projection (toggle) vs raw lon/lat         | ✓    | partial     |
| Custom width                                            | ✓    | some        |
| Custom fill / stroke / stroke-width / point-radius      | ✓    | some        |
| Custom / transparent background                         | ✓    | some        |
| Clean, compact, standalone SVG (Illustrator/Inkscape)   | ✓    | ✓           |
| 100% client-side / nothing uploaded                     | ✓    | GeoUtil/geojson2svg only |
| Runs in chat + CLI + page (3 surfaces)                  | ✓    | ✗ (web only)|

## Gaps closed this pass

The tool was built to parity in one pass: width / fill / stroke / stroke-width / point-radius /
background / projection knobs all exposed across the three surfaces, holes rendered with
`fill-rule="evenodd"`, degenerate inputs (single point, vertical/horizontal line) re-centred so they
still render, and lat clamped to the Mercator-valid range to avoid pole infinities.

## Out-of-model (intentionally not built)

- **Raster export (PNG/JPEG/PDF/SVGZ)** — competitors export bitmaps/gzip; the page output surface is
  text/SVG. A raster step would belong to the separate `svg-to-png` tool, which already exists.
- **Drag-and-drop file upload of a `.geojson` file** — the pure-text page takes pasted GeoJSON in a
  textarea (consistent with the other text tools); a file-upload surface is a different Input kind.
- **Live interactive pan/zoom map preview** — out of scope for a deterministic compute tool; the output
  is a static, embeddable SVG.
- **Reprojection between arbitrary CRS / EPSG codes** — would need a projection library; we offer the two
  most-used options (Web-Mercator and raw WGS84 lon/lat plot).

No competitor copy, branding, or trademarks were used.

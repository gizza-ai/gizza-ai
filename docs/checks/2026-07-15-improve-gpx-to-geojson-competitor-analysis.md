# gpx-to-geojson — competitor analysis (2026-07-15)

Scope: tools that convert GPS track/waypoint formats (GPX, KML) into GeoJSON, and
the reverse. All observations are paraphrased from public product behaviour and a
widely-used open-source conversion library's documentation; no competitor copy,
branding, or trademarks are reproduced.

## Competitors reviewed

1. **A well-known open-source "KML/GPX to GeoJSON" JS library** (the de facto
   reference for how GPX/KML elements map to GeoJSON) — documents the exact
   element mapping: GPX `trk`/`trkseg`/`trkpt` → LineString, `wpt` → Point,
   standard tags (`name`, `cmt`, `desc`, `link`, `time`, `keywords`, `sym`,
   `type`) become properties; KML `Point`/`LineString`/`Polygon` → matching
   geometry, `MultiGeometry` → GeometryCollection, `ExtendedData`/`SimpleData` →
   arbitrary properties, `TimeSpan`/`TimeStamp` → temporal properties, and
   `Style`/`styleUrl` → a best-effort simplestyle-ish color/width mapping (the
   docs are explicit that style conversion is "best effort", not a full replica
   of KML's non-semantic styling system).
2. **A client-side, offline-first "GPX to GeoJSON" web tool** — file upload +
   drag-and-drop, a live map preview of the converted GeoJSON, and a download
   button; explicitly markets "runs 100% in your browser, nothing uploaded."
3. **A GIS-focused cloud "KML/GPX to GeoJSON" converter** — file upload +
   drag-and-drop only (no paste-text), supports huge coordinate-system libraries
   and batch/ZIP multi-file conversion, packages output as a ZIP for download.
   No reverse (GeoJSON→GPX) direction offered.
4. Two further generalist converters ("supports KML, GPX, GeoJSON, CSV with
   automatic format detection", and a KML-only variant) — both confirm
   auto-detection of input format from content as the norm (no explicit
   "which format is this?" selector), and both are one-directional (into
   GeoJSON only; none advertise a GeoJSON→GPX/KML reverse path).

## Table-stakes → where each lands in our model

| Table-stake capability | Decision |
| --- | --- |
| GPX `trk`/`trkseg`/`trkpt` → LineString (MultiLineString for multi-segment tracks) | **In model** — `core::gpx_to_geojson`. |
| GPX `wpt` → Point feature | **In model**. |
| GPX `rte`/`rtept` → LineString feature | **In model**. |
| GPX standard properties (name, cmt, desc, link, time, keywords, sym, type) | **In model** — copied onto `properties` when present. |
| GPX elevation (`ele`) as the position's third coordinate | **In model**. |
| GPX per-point timestamps as a `coordTimes` property array (matches the reference library's own naming, for interchange familiarity) | **In model**. |
| KML `Point`/`LineString`/`Polygon` → matching GeoJSON geometry | **In model**. |
| KML `MultiGeometry` → `GeometryCollection` | **In model**. |
| KML `name`/`description` → properties | **In model**. |
| KML `ExtendedData`/`SimpleData` → arbitrary properties | **In model**. |
| KML `TimeSpan`/`TimeStamp` → temporal properties | **In model**. |
| KML `Style`/`styleUrl`/`StyleMap` → simplestyle-spec color/width properties (`stroke`, `stroke-width`, `stroke-opacity`, `fill`, `fill-opacity`, `marker-color`) | **In model**, toggle via `include_styles` (default true) — inline `<Style>` AND `<styleUrl>`-referenced shared styles/`StyleMap` are both resolved. |
| Auto-detect input format from content (no manual "which format" selector) | **In model** — `input` is sniffed for a `<gpx`/`<kml` root vs. JSON; no `input_format` param needed. |
| Reverse: GeoJSON → GPX (Point→wpt, LineString/MultiLineString→trk/trkseg) | **In model** — `output_format = "gpx"`. None of the reviewed competitors offer this direction at all, but it's the explicit backlog ask ("…and back"), so we build it rather than defer it. |
| Live interactive map preview with a basemap | **Out of model** — gizza already ships a purely local map-render tool (`blocks/geojson-to-svg`, SVG from GeoJSON, no tile server); duplicating a preview here would either (a) reimplement that tool's job or (b) require a live tile server, breaking the "runs entirely offline/private" guarantee every gizza page states. Documented in this tool's FAQ as "pipe the GeoJSON output into geojson-to-svg for a visual preview." |
| Batch / multi-file ZIP upload+download | **Out of model** — the gizza page model is a single text/file field with one text/media output; there is no multi-file input or ZIP output surface (same class of gap as the multi-input-ffmpeg skips). |
| 7,000+ coordinate reference systems / reprojection | **Out of model** — GeoJSON's spec mandates WGS-84 lon/lat, and GPX/KML are WGS-84 by construction; no reprojection is needed for this conversion, so it is not a real gap. |
| Reverse to KML (GeoJSON → KML) | **Out of model (this tool)** — no reviewed competitor round-trips to KML either; KML styling is lossy to reconstruct faithfully from bare GeoJSON properties, and GPX is the far more common re-import target (GPS devices, Strava-style services). Deferred; revisit only if there's demand and a clear styleUrl encoding to target. |
| Polygon → GPX | **Partially in model** — GPX has no polygon primitive, so a Polygon's exterior ring is emitted as a closed `trkseg`; interior rings (holes) are dropped. Documented as a stated limitation, not silently lossy. |

## Notes / honesty

- Format auto-detection, not a format-choice dropdown, matches what every
  reviewed competitor does — it also removes an entire parameter from our
  descriptor.
- The reference JS library's own docs call KML style conversion "best effort";
  our implementation resolves inline `<Style>` and `styleUrl`-referenced
  `<Style>`/`<StyleMap>` into simplestyle-spec properties, which is the same
  scope, independently implemented in Rust (no code, constants, or wording
  copied from that library).
- No competitor copy, UI text, or branding was copied anywhere in this tool's
  page or descriptor.

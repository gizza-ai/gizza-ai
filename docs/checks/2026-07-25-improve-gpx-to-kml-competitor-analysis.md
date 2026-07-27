# gpx-to-kml — competitor analysis (2026-07-25)

Scan done BEFORE implementing. One WebSearch ("GPX to KML converter online
Google Earth options"), then skimmed the top real competitor tools. All
observations are paraphrased; no competitor copy, branding, or trademarks are
reproduced. Neutral references only.

## Competitors reviewed

1. **MapKmlTools (mapkmltools.com/gpx-to-kml)** — the most feature-rich
   browser-based converter. Offers KML *and* KMZ output, a styling panel
   (path color, line thickness, structure/opacity, waypoint icon color) with a
   live map preview, and states it converts `<trk>`/`<rte>`/`<wpt>` to KML
   LineStrings / Placemarks / Points. 100% client-side.
2. **The Ride Atlas (therideatlas.com/tools/gpx-converter/gpx-to-kml)** —
   drag-and-drop, fully client-side ("file never leaves your browser"). Exposes
   a styling panel: track/route color (default a red `#ef4444`), line width
   (default 4), opacity (default 80%), separate waypoint icon color (default a
   blue `#3b82f6`). Preserves timestamps and elevation (altitude for 3D). KML
   and KMZ output. Shows route stats (distance, elevation gain).
3. **Dawarich / AnyConv / MyGeodata / gpx2kml.com** — simpler converters:
   upload → download KML, little or no styling. AnyConv/MyGeodata are
   server-side batch converters; Dawarich and gpx2kml are minimal
   browser-based one-click tools. Free KML Tools additionally offers a
   ground-level "Track" vs an aerial "Tour" (flythrough) output mode.

## Table-stakes parameters (with defaults) and model-fit

| Capability | Typical default | In-model? | Decision |
|---|---|---|---|
| Track/route line color | red (`#ef4444`) | ✅ | `line_color`, color control, default `#ef4444` |
| Line width | 4 | ✅ | `line_width`, slider 1–20, default 4 |
| Line opacity / transparency | 80% | ✅ | `line_opacity`, slider 0–100, default 80 |
| Waypoint icon color | blue (`#3b82f6`) | ✅ | `waypoint_color`, color control, default `#3b82f6` |
| Preserve trk→LineString, rte→LineString, wpt→Point | — | ✅ | core behavior |
| Preserve elevation (altitude for 3D) | on | ✅ | elevation carried into `lon,lat,ele`; `altitude_mode` enum |
| Altitude interpretation in Google Earth | clamp to ground | ✅ | `altitude_mode` = clamptoground / absolute / relativetoground |
| Preserve timestamps | on | ✅ | waypoint `<TimeStamp>` + track `<TimeSpan>` when present |
| Named waypoints / descriptions | on | ✅ | name + description carried through |
| Document name | filename | ✅ | `document_name`, optional, falls back to GPX `<name>` |

## Out-of-model (listed, not built)

- **KMZ output** — KMZ is just a ZIP-compressed KML. The page/CLI text surface
  renders text, and the produced KML opens directly in Google Earth, so KML is
  the primary output. KMZ (binary) is a separate media-envelope shape; noted,
  not built here.
- **Live map preview / route stats (distance, elevation gain)** — rendering UI
  that belongs to a site front-end, not the pure converter; the sibling
  `gpx-analyzer` tool already computes track stats.
- **Aerial "Tour" (gx:Tour flythrough) output** — niche animation mode; the
  converter emits a standard ground/absolute track that Google Earth can fly on
  its own.
- **Bulk / multi-file conversion** — the tool takes a single GPX document
  (matches the single-input page model).

## Design descriptor (all in-model params)

`gpx` (required), `line_color`, `line_width`, `line_opacity`, `waypoint_color`,
`altitude_mode`, `document_name`. Every param has a `.describe()`; the two
color params use the `color` page control, the two numeric params use sliders,
`altitude_mode` is a `Param::enumv` → `<select>`.

### Correctness note recorded during design

KML `<color>` is **`aabbggrr`** (alpha, blue, green, red) — the *reverse* byte
order of the familiar CSS `#rrggbb`, with an alpha byte prepended. The
converter must translate the user's `#RRGGBB` + opacity% into KML `AABBGGRR`;
getting this backwards renders every track the wrong color in Google Earth.

# tcx-to-gpx — competitor analysis (2026-07-17)

Snapshot taken while building `tcx-to-gpx` (Garmin Training Center XML → GPX 1.1).
Competitors analyzed for **ideas/features only** — no copy, branding, or trademarks were
reused. All conclusions are paraphrased.

## What a TCX file holds (that GPX conversion must handle)

TCX nests data as `Activities/Activity/Lap/Track/Trackpoint` (and, for routes,
`Courses/Course/Track/Trackpoint`). Each `Trackpoint` may carry `Position`
(`LatitudeDegrees`/`LongitudeDegrees`), `AltitudeMeters`, `Time`, `DistanceMeters`,
`HeartRateBpm/Value`, and `Cadence`. GPX 1.1 maps lat/lon → `<trkpt>`, altitude →
`<ele>`, time → `<time>` cleanly; heart-rate/cadence have no core GPX slot and need the
Garmin `TrackPointExtension` schema (or get dropped). Laps have no GPX equivalent.

## Top 3 competitors (paraphrased)

### 1. Dawarich (dawarich.app/tools/tcx-to-gpx)
- Single-file, browser-local TCX→GPX; part of a broader converter hub.
- No options exposed. Preserves lat/lon, timestamps, elevation, sport/distance metadata.
- **Drops HR/cadence/power** ("require GPX extensions"). Drag-and-drop upload, no preview.
- Angle: privacy-first, cross-ecosystem portability (Google Earth/QGIS).

### 2. Sport-Calculator (sport-calculator.com/converters/tcx-to-gpx)
- Single-file TCX→GPX, **GPX 1.1-compliant** output; multi-activity/multi-lap aware.
- No user-facing options, but **preserves HR/cadence/power via GPX extensions** — best fidelity.
- 2 MB input cap. Drag-and-drop, three-step flow, single download.
- Angle: "100% private", open-standard/universal-compatibility framing.

### 3. GMaps To GPX (gmapstogpx.com/tools/tcx-to-gpx)
- Browser-local TCX→GPX with a companion GPX viewer and reverse direction.
- No options. Extracts trackpoints + elevation; **drops HR/cadence**; timestamps unclear.
- Angle: TCX is "Garmin-locked", GPX is the universal escape hatch.

## Table-stakes (all three)
- Free, no login, browser-local, explicit no-upload/privacy messaging.
- Preserve trackpoints, elevation, and (mostly) timestamps.
- Handle multiple activities/laps without crashing.

## Gaps + how this tool addresses them (in-model)
- **Paste-text input** — none of the three accept pasted XML; this tool does (verifiable on
  the page/CLI without a binary upload). *Built.*
- **HR/cadence retention via `TrackPointExtension`, with a user toggle** — matches
  Sport-Calculator's fidelity, beats Dawarich/GMaps, and adds control the others lack.
  `include_extensions` (default on). *Built.*
- **Multi-activity + multi-lap → one `<trk>` per activity/course, one `<trkseg>` per TCX
  `<Track>`** — preserves structure instead of flattening. *Built.*
- **Metadata preserved:** `Sport` → `<type>`, Course `<Name>`/Activity `<Id>` → `<name>`,
  earliest point time → `<metadata><time>`. *Built.*
- **No input-size cap** (Sport-Calculator caps at 2 MB) — runs fully locally with no server
  limit. *Built.*
- **State exactly what is kept/dropped** on the page (limits section + FAQ). *Built.*

## Considered, not built (out-of-model or out-of-scope for this pass)
- Batch/multi-file zip download — the page is a single paste/field surface; multi-file upload
  UI is out of scope for this converter.
- Live map preview + stats summary (points/distance/elevation gain) — a rendering feature, not
  a conversion capability; considered, deferred.
- GPX→TCX reverse direction — this tool is one-directional TCX→GPX by design (the backlog
  entry); the reverse is a separate tool.
- Power (`Watts` extension) — TCX power lives in vendor `Extensions` namespaces with no single
  standard element; HR + cadence (standard TCX elements) are the reliable, portable subset.

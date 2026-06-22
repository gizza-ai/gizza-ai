# gpx-analyzer — competitor analysis & improvement check (2026-06-22)

## Tool

`blocks/gpx-analyzer` — pure-Rust (`quick-xml`) GPX track analyzer. Parses
`<trkpt>`/`<rtept>`/`<wpt>` points and reports distance, elevation, duration,
speed, pace, splits, and sensor extensions. Runs locally on all surfaces
(chat block, CLI, standalone page). No network, no model, no file upload.

## Surfaces verified

- **Chat block:** `wafer build` → `OK gizza-ai/gpx-analyzer v0.1.0 (388 KiB)` —
  the wasm32-wasip1 block instantiates.
- **CLI:** `gizza tool gpx-analyzer gpx="…"` returns the full JSON stats object
  (distance, elevation gain/loss, min/max, start/end time, duration, avg & max
  speed, avg pace, HR/cadence/power/temp, per-km & per-mile splits).
- **Page:** Playwright `tool-page-gpx-analyzer.spec.ts` passes — pasting a GPX
  track renders distance, elevation, duration, max speed, and pace in-browser.
- **Unit tests:** 12 core/block tests pass, including the schema drift-guard.

## Competitors surveyed (top 5)

| Tool | Stats offered |
|------|---------------|
| GPX Viewer (GlandNav) | distance, time, speed, elevation gain, elevation chart, map |
| GPX Viewer (Steps App) | distance, elevation gain, duration, average pace, per-point speed/altitude, map |
| Mappr GPX Viewer | distance, elevation gain & loss, min/max elevation, duration, average speed, map |
| GpxOverlay Activity Analyzer | distance, elevation gain & loss, avg & max speed, HR, cadence, power, temperature, profile |
| uTrack / ViewMyGPX / trackreport.net | distance, elevation gain/loss, duration, pace, gain/loss profile, splits |

Common themes across all: **distance (km + mi), elevation gain/loss + min/max,
duration, average speed/pace, max speed, sensor channels (HR/cadence/power/
temperature), and per-distance splits.** The map/3D visualization and elevation
*charts* are the headline visual features.

## Gap diff (vs. the initial build) and what was closed

The first build covered distance (km/mi), elevation gain/loss + min/max,
duration, average speed (km/h + mph) and average pace (min/km + min/mi). The
competitor sweep surfaced these missing in-model capabilities, all of which are
pure XML/number work and were added:

- **Max speed (km/h + mph)** — max instantaneous speed between consecutive timed
  points. (GpxOverlay, GlandNav.)
- **Start / end time (ISO-8601)** — exposed from the first/last timestamps.
- **Per-km and per-mile splits** — each split's duration, pace (MM:SS), and
  elevation gain, with the boundary timestamp linearly interpolated inside the
  crossing segment (constant-pace assumption). Split durations sum to the total
  duration. (uTrack / trackreport / common running-tool feature.)
- **Sensor extensions** — average & maximum **heart rate, cadence, power, and
  temperature** parsed from the Garmin `TrackPointExtension` (and bare
  `<hr>/<cad>/<power>/<atemp>` variants). (GpxOverlay's headline differentiator.)

## Out-of-model gaps (NOT built — documented only)

- **Map rendering / 2D & 3D track visualization** — needs an interactive
  slippy-map / WebGL canvas; the gizza page surface renders text/number/media,
  not an interactive map. Out of the pure-compute text model.
- **Elevation / pace charts** — a rendered line chart would need a charting
  surface; the page driver shows a text result. (gizza has separate chart blocks,
  but this tool's surface is a text report.) Could be a future media-output
  variant; not in scope for a stats report.
- **Drag-and-drop file upload** — the page uses a paste-the-XML text field
  (consistent with eml-parse and other text-in tools); a `.gpx` file picker would
  need the `AssetKind`/file-input page path, which is a framework change.

No competitor copy, branding, or trademarks were used.

## Result

Built and verified end-to-end on all three applicable surfaces. All
competitor-comparable, in-model statistics are now covered.

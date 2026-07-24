# gpx-split — competitor analysis (2026-07-23)

Function: split one GPX track into multiple segments by distance, elapsed time,
or detected stop/pause gaps, and emit either a new multi-segment GPX or a text
summary. All paraphrased; no competitor copy/branding reused.

## Competitors scanned

1. **GPXto — GPX Splitter** (gpxto.com/gpx-split) — browser tool. Split modes:
   by **distance** (e.g. every 5 km or miles), by **number of points** (device
   caps), by **number of equal stages**. No time-interval or stop-gap mode.
   Distance unit selectable km/mi.
2. **GPX Rescue** (gpxrescue.eu) — browser. Splits one long activity into
   multiple by **stops** (detects restaurant/break stops, converts to pauses)
   or by **distance**. Emphasis on stop detection.
3. **split-gpx-track** (hrehfeld, GitHub) — CLI. Detects **pauses** and splits;
   options `--pause-duration-min` (min pause length) and `--pause-velocity-max`
   (treat near-zero speed as a pause), plus include/exclude short/slow tracks.
4. **gpxslicer** (PyPI) — CLI. Split by **distance** — `-d 5000` splits every
   5000 m; distance in metres.
5. **GPSBabel `track` filter** — CLI. `split` by **time gap** or **distance
   gap** between successive points (start a new track when the gap exceeds a
   threshold) — the classic stop/gap split.

## Table-stakes → decision

| Capability | Competitors | In gpx-split? |
|---|---|---|
| Split by distance threshold | GPXto, gpxslicer, GPX Rescue | ✅ `mode=distance` + `distance` + `unit` (km/mi) |
| Split by elapsed time per segment | (implied; GPSBabel time) | ✅ `mode=time` + `time_min` |
| Split at detected stop/pause gaps | GPX Rescue, split-gpx-track, GPSBabel | ✅ `mode=stops` + `stop_gap_s` (time gap between points) |
| Distance unit km/mi | GPXto | ✅ `unit` enum |
| Output a valid multi-segment GPX | all GPX tools | ✅ `output=gpx` (N named `<trk>`) |
| Per-segment summary (distance/duration) | GPXto preview map | ✅ `output=summary` text table |
| Preserve elevation + timestamps | all | ✅ `<ele>`/`<time>` carried per point |
| Runs locally, no upload | GPXto, GPX Rescue, Ride Atlas | ✅ pure Rust/WASM in browser |

## Out of model (listed, not built)

- **Interactive map preview / click-to-split at a chosen point** (GPXto, Ride
  Atlas, gpx.studio scissors) — needs a map UI; this is a deterministic
  text-in/text-out tool.
- **Split into N equal stages / by point count** — GPXto extras; kept the three
  domain-standard modes (distance/time/stops) to stay focused. Could be added
  later as extra `mode` values.
- **Pause-by-velocity threshold** (split-gpx-track `--pause-velocity-max`) —
  approximated by the simpler, more robust **time-gap** stop detection
  (`stop_gap_s`), which is what GPSBabel and most recorders use (a paused
  recording shows a large timestamp gap). Velocity smoothing is noise-sensitive
  and out of scope.
- **Sensor extension preservation** (HR/cadence/power) — v1 preserves geometry
  (lat/lon/ele/time); extensions are dropped. Documented as a limit.

## UX patterns matched

- Preset **example chips** for the common splits (every 5 km, every 30 min,
  detect stops) — mirrors competitors' one-click presets.
- Enum **select** controls with friendly labels for mode/unit/output.
- Km/mi unit toggle as an enum select.

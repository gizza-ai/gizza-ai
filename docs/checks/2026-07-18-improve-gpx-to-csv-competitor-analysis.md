# gpx-to-csv — competitor analysis (2026-07-18)

Tool: **gpx-to-csv** — extract track/route/waypoint coordinates, elevation, timestamps
(and sensor extensions) from a GPX file into CSV. Pure-Rust `quick-xml`, browser-local.

## Competitor scan (paraphrased — no copy/branding reproduced)

WebSearch: "GPX to CSV converter online extract track points coordinates elevation timestamp".
Skimmed the top real, reachable tools:

1. **BentoUtils GPX-to-CSV** (browser-local, no upload). Point-type selector
   (track points / waypoints / both). "Choose which columns to include." Preview row
   count (10/50/100/all). Extracts lat, lon, altitude (m), ISO-8601 time, plus GPX
   extension channels — heart rate (Garmin/Polar/Wahoo), cadence, power, temperature —
   and named waypoints with descriptions/symbols. Delimiter/header not exposed (fixed).
2. **Takeout-Tools GPX-to-CSV** (browser-local). Columns: name, latitude, longitude,
   elevation, timestamp, description, plus any GPX extension fields. One row per
   coordinate. Handles waypoints + track points. Preset defaults; no delimiter/point-type
   toggles exposed. Drag-drop / click, auto-convert, download.
3. **GPXto GPX-to-CSV** (browser-local). Delimiter choice: comma / semicolon / tab /
   pipe. Header row toggle. Time format: ISO / local / Unix. Point types: tracks, routes,
   waypoints. Elevation preserved; speed can be derived from timestamps. Upload → configure
   → convert → download; optional map preview before download.

(MyGeodata Cloud, GPS Visualizer, Maparz also seen — server-side / GIS-batch oriented;
Maparz emits X/Y/elevation/timestamp attribute rows.)

## Table-stakes → in-model / out-of-model

| Feature | Competitors | Decision |
| --- | --- | --- |
| One row per point, lat/lon columns | all | in-model — core output shape |
| Point-type filter (track/route/waypoint/all) | Bento, GPXto | **in-model** → `points` enum, default `all` |
| Elevation column (metres) | all | in-model — `elevation_m`, emitted when present |
| Timestamp column | all | in-model — `time` |
| Time format ISO / Unix | GPXto | **in-model** → `time_format` enum `iso`\|`unix`\|`none` |
| Time format "local" | GPXto | **out-of-model** — needs a timezone/DST database; GPX time is UTC. Listed, not built. |
| Delimiter comma/semicolon/tab/pipe | GPXto | **in-model** → `delimiter` enum, default `comma` |
| Header-row toggle | GPXto | **in-model** → `header` boolean, default `true` |
| Sensor extensions (HR, cadence, power, temp) | Bento | **in-model** — spike confirmed pure `quick-xml` parse of Garmin TrackPointExtension (already proven in gpx-analyzer); columns emitted only when present |
| Waypoint name / description | Bento, Takeout | in-model — `name` column (point `<name>`) |
| Derived speed from timestamps | GPXto | **in-model** → `speed` boolean adds `speed_kmh` (haversine ÷ Δt, per-segment) |
| Interactive map preview | GPXto | out-of-model — no map renderer; out of scope for a text tool |
| Choose/reorder arbitrary columns UI | Bento | **considered, rejected** — a drag-reorder column-picker is UX bloat for a paste-in tool; instead columns are emitted only when the data is present + optional speed |
| 5 GB upload / ZIP / batch | MyGeodata | out-of-model — server batch; this is browser-local paste/upload |

## Feasibility spike

A pure-Rust `quick-xml` streaming parse of GPX (`<trkpt>`/`<rtept>`/`<wpt>`, `<ele>`,
`<time>`, Garmin `TrackPointExtension` sensor tags) is already proven in `gpx-analyzer`
on this box — reused the same local-name matching + self-contained ISO→epoch parser.
Haversine per-segment speed is trivial pure math. No out-of-model dependency for any
built feature.

## Descriptor (built)

- `gpx` string (required)
- `points` enum `all`\|`track`\|`route`\|`waypoint` (default `all`)
- `delimiter` enum `comma`\|`semicolon`\|`tab`\|`pipe` (default `comma`)
- `header` boolean (default `true`)
- `time_format` enum `iso`\|`unix`\|`none` (default `iso`)
- `speed` boolean (default `false`)

Columns: `point_type,name,latitude,longitude,elevation_m,time[,speed_kmh]` + sensor
columns (`heart_rate_bpm,cadence_rpm,power_w,temperature_c`) appended when present.
RFC-4180 escaping (quote fields containing the delimiter/quote/newline).

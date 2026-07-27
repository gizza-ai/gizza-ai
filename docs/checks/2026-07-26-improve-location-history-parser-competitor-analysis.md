# Competitor analysis — location-history-parser (2026-07-26)

Scan done BEFORE implementation. Goal: parse Google Takeout / Dawarich
location-history JSON into a per-day summary of places visited + distance
traveled. All notes below are paraphrased; no competitor copy, branding, or
trademarks are reproduced.

## Competitors surveyed

1. **A forensic Takeout location-history parser (open-source).** Parses classic
   Records-style exports and the semantic "place visit / activity segment"
   monthly files. Emphasis on faithful extraction of every record with
   timestamps and raw coordinates. Table-stakes taken from it: accept both the
   `latitudeE7`/`longitudeE7` integer encoding and ISO timestamps; understand
   place-visit vs activity-segment objects.

2. **A Takeout location `.json` parser CLI/library.** Reads the `locations`
   array from the classic export, normalizes `timestampMs` / `timestamp`, and
   emits tabular rows. Table-stakes: handle both millisecond-epoch and ISO time
   strings; tolerate extra fields.

3. **A general Google-Takeout parser (multi-datatype).** Offers a "summary"
   action and JSON output, filtering by data type (place visit, activity,
   location). Table-stakes: a summary mode (counts/aggregates), JSON output
   option.

4. **A visited-places extractor (online geoprocessing).** Uploads Takeout KML or
   JSON and lets the user pick parameters to select which stationary points
   count as a "visited place" (dwell/proximity thresholds). Table-stakes: a stop
   / dwell threshold and a proximity radius to turn a raw GPS stream into
   discrete places; CSV output.

5. **A privacy-first Google Timeline visualizer (Dawarich ecosystem).** Imports
   the newer on-device Timeline JSON (semantic segments with `visit` /
   `activity` / `timelinePath`, `latLng` strings, `distanceMeters`) and the
   classic semantic files entirely in the browser; stresses that nothing is
   uploaded. Table-stakes: support the new on-device Timeline shape and the
   `distanceMeters` an activity already carries; strictly local processing.

## Table-stakes → decision (each lands in the descriptor or is listed out-of-model)

| Capability | Decision |
|---|---|
| Classic `Records.json` (`locations[]`, `latitudeE7`/`longitudeE7`, `timestampMs`/`timestamp`) | **In** — parsed into a point stream. |
| Semantic Location History (`timelineObjects[]` place-visit + activity-segment) | **In** — visits→places, activity `distance`/`distanceMeters`→distance. |
| New on-device Timeline (`semanticSegments[]` with `visit`/`activity`/`timelinePath`, `latLng` strings) | **In** — parsed for visits, activity distance, and path points. |
| Dawarich / generic point arrays (`latitude`/`longitude`/`timestamp`) | **In** — point stream (also GeoJSON Point FeatureCollection with a time property). |
| Distance traveled per day | **In** — from activity `distanceMeters` when present, else great-circle (haversine) sum of consecutive points. |
| Places visited per day | **In** — named visits when the export has them; stop-detection (dwell + radius) for raw GPS streams. |
| Stop/dwell threshold + proximity radius | **In** — `min_stay_min` + `place_radius_m` params (used for raw point streams). |
| Local-time bucketing | **In** — `utc_offset` hours param so days match the user's local calendar. |
| Unit toggle (km / miles) | **In** — `unit` enum. |
| Output as text summary / CSV / JSON | **In** — `output` enum. |
| Strictly local / no upload | **In** — gizza runs entirely in-browser wasm; no network. |
| ZIP-of-Takeout upload + interactive map/heatmap | **Out-of-model** — this is a single-JSON, text-in/text-out tool; unzipping a Takeout archive and rendering an interactive map need a file-tree UI + map engine that the page model doesn't provide. Users paste one JSON file (Records/Semantic/Timeline/Dawarich). |
| KML input | **Out-of-model (listed, not built)** — the picked scope is JSON; KML is an XML format outside this parser. Convert KML separately first. |
| Reverse-geocoding unnamed stops to street addresses | **Out-of-model** — needs a network geocoding service; gizza is offline. Unnamed stops are reported by coordinate. |

## Notes / design choices

- Distance uses the great-circle (haversine) formula on WGS84 lat/lon,
  R = 6,371,000 m. For semantic activity segments that already carry a
  `distance`/`distanceMeters`, that value is used directly (matches Google's own
  figure) and haversine is the fallback only for raw point streams.
- Stop detection is a deterministic greedy anchored cluster: consecutive points
  within `place_radius_m` of the cluster anchor that span at least
  `min_stay_min` are one visited place. Simple and explainable; documented on
  the page as a heuristic for raw GPS streams (semantic exports use their own
  named places instead).
- All copy, examples, and FAQ on the page are original.

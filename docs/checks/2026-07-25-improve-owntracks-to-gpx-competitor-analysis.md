# Competitor analysis — owntracks-to-gpx (2026-07-25)

Tool function: convert OwnTracks location exports (JSON or Recorder `.rec`) into a standard GPX track. Notes are paraphrased; no competitor copy, branding, or trademarks are copied.

## Competitors scanned

1. **OwnTracks Recorder / `ocat` workflows** — exports OwnTracks data as JSON and related formats, but users still commonly need a GPX conversion step for map import.
2. **GPSBabel format converter** — broad GPS conversion suite with GPX as a target. Table-stakes: standard GPX XML, time/elevation preservation, filtering/simplification options. Heavy desktop CLI; not a browser-local OwnTracks-focused form.
3. **GPS Visualizer / online map converters** — web converters that accept GPS-ish data and output GPX/KML/maps. Table-stakes: paste/upload data, name tracks, preserve timestamps, downloadable GPX. Upload/server model is out-of-scope here.
4. **Ad-hoc OwnTracks-to-GPX scripts** — small scripts in the community parse `.rec` or JSON and emit `<trkpt>`. Table-stakes: skip non-location messages, handle `tst`, include elevation, tolerate Recorder line format.

## Table-stakes decisions

| capability | decision | tag |
| --- | --- | --- |
| JSON array / Recorder API object / single object | Parse all three, plus NDJSON | IN-MODEL |
| `.rec` line format | Parse JSON payload after timestamp/type columns | IN-MODEL |
| Standard GPX 1.1 | Emit `<gpx><trk><trkseg><trkpt>` with schema namespace | IN-MODEL |
| Timestamps | Convert `tst` epoch seconds to UTC ISO; fallback to `.rec` ISO column | IN-MODEL |
| Elevation | Map `alt` to `<ele>` | IN-MODEL |
| OwnTracks metadata | Optional extension namespace for `acc`, `vel`, `cog`, `batt`, `tid` | IN-MODEL |
| Filtering | `max_accuracy_meters` drops imprecise fixes | IN-MODEL |
| Trip segmentation | `segment_gap_minutes` starts new `<trkseg>` on large gaps | IN-MODEL |

## Out-of-scope / not built

- Map preview, route simplification, smoothing, map matching, and geocoding need heavier GIS logic or map/network resources; listed as out-of-scope.
- Batch file uploads and cloud account integrations are outside the local text-in/text-out gizza model.
- KML/FIT/TCX outputs are distinct converters; this tool focuses on GPX.

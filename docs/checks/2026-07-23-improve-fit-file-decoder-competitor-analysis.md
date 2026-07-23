# fit-file-decoder — competitor analysis (2026-07-23)

Snapshot of how the leading FIT-file decoders/converters behave, used to set the table-stakes
for this tool. Paraphrased notes only — no competitor copy, branding, or trademarks reproduced.

## Tools skimmed (top real results)

1. **MyGeodata Cloud — FIT to GPX** (mygeodata.cloud/converter/fit-to-gpx) — upload a `.fit`,
   pick GPX out, download. Generic geodata converter; supports many GPS formats. Server-side.
2. **GOTOES — Convert FIT Files to CSV** (gotoes.org) — upload a Garmin `.fit`, get a CSV with
   "all of the record fields" (one row per record message: time, position, ele, distance, speed,
   HR, cadence, power, temperature, etc.). Aimed at Strava power users.
3. **Sport-Calculator — FIT to GPX / FIT to CSV** (sport-calculator.com) — in-browser, "nothing
   uploaded to servers", preserves GPS track, elevation and heart rate; output ready to re-upload
   to Strava/Komoot/RideWithGPS. Also a FIT→CSV variant.

Also seen: fitfiletools.com (edit + convert to GPX, no account), Convert.Guru (drag-drop FIT →
analyze + convert to GPX/TCX/CSV), AI Training Plan (FIT → GPX/TCX/CSV, choose which fields to
include, local/private), GpxOverlay (FIT → GPX instantly).

## Table-stakes (and where each lands)

| Capability | Competitors | Our decision |
|---|---|---|
| Read binary Garmin/ANT FIT (header, definition + data records, LE/BE, dev fields) | all | **in-model** — bounded minimal pure-Rust FIT parser |
| Export **GPX** (track: lat/lon, ele, time, + HR/cadence/power extensions) | MyGeodata, Sport-Calc, GpxOverlay | **in-model** — `format=gpx` |
| Export **CSV** (one row per record, all common record fields) | GOTOES, Sport-Calc, AI Training Plan | **in-model** — `format=csv` |
| **Summary / analyze** view (sport, distance, duration, HR/speed/power, bounding box) | Convert.Guru | **in-model** — `format=summary` (default) |
| Preserve elevation, timestamp, heart rate, cadence, power, speed, distance | all serious ones | **in-model** — decoded from record msg fields 253/0/1/2/3/4/5/6/7 (+ enhanced 73/78) |
| Session/lap/activity summary (sport, totals, averages) | GOTOES, Convert.Guru | **in-model** — session (msg 18) fields surfaced in summary |
| Local / private processing, nothing uploaded | Sport-Calc, AI Training Plan | **in-model** — runs fully in-browser (page) / offline (CLI); no network |
| Drag-and-drop the raw `.fit` binary on the page | most | **out-of-model (input shape)** — the generic pure-tool page has no binary file-upload control (only ffmpeg tools upload files). We accept the FIT bytes as **base64 text**, the verifiable shape that fits the current page + CLI + chat model. Documented on the page. |
| Export **TCX** | Convert.Guru, AI Training Plan | **out-of-model (scope)** — GPX already covers the "re-upload to a GPS app" use case; a second XML writer adds surface without new capability. Listed, not built. |
| **Choose which fields** to include | AI Training Plan | **out-of-model (scope)** — CSV emits the full fixed record-field set; per-column selection is a future param. |
| **Edit** FIT (trim, merge, time-shift) | fitfiletools | **out-of-model** — separate tools, not a decoder. |
| Batch / multi-file | some | **out-of-model** — one file per run (chat/CLI/page are single-input). |

## In-model params designed from this scan

- `data` (string, base64 of the FIT file bytes; required; multiline textarea on the page).
- `format` (enum `summary` | `csv` | `gpx`; default `summary`) — the three export shapes above.

Size cap: decoded FIT payload is capped (8 MiB) to stay inside the wasm sandbox — stated on the
page and enforced with a clear error.

## UX controls matched

- `format` renders as a `<select>` with friendly labels (Summary / CSV / GPX).
- `[[example]]` preset chips prefill a real base64 FIT sample for one-click "Try:".
- Worked example (base64 in → GPX/CSV/summary out) + ≥3 FAQs on the page.

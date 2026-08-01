# nmea-to-csv — competitor analysis (2026-07-30)

Snapshot taken while building the new `nmea-to-csv` tool. NMEA 0183 is the raw
serial GPS/GNSS log format (comma-delimited `$GPGGA` / `$GPRMC` "sentences" with a
trailing `*XX` checksum) — a distinct input from GPX (XML), which the existing
`gpx-to-csv` block handles. No existing gizza block parses NMEA, so this is a new
tool, not a semantic duplicate. All findings below are **paraphrased** — no
competitor copy, branding, or trademarks were reproduced.

## Competitors surveyed (top 5 reachable)

| # | tool | url |
|---|------|-----|
| 1 | MyGeodata Cloud — NMEA to CSV | https://mygeodata.cloud/converter/nmea-to-csv |
| 2 | Convert.Guru — NMEA Converter | https://convert.guru/nmea-converter |
| 3 | Online NMEA Analyser (Bluecover) | https://swairlearn.bluecover.pt/nmea_analyser |
| 4 | Petrichor-Labs `nmea_data_convert` (pynmea2, OSS) | https://github.com/Petrichor-Labs/nmea_data_convert |
| 5 | freenmea.net free NMEA tools | http://freenmea.net/ (unreachable at scan time — replaced with #4) |

freenmea.net refused the connection during the scan, so it was replaced by the
Petrichor-Labs OSS library, which documents exact per-sentence CSV columns.

## Per-competitor profile (paraphrased)

**MyGeodata Cloud** — Upload-based `.nmea/.nme/.log/.txt` → CSV (also GPX/GPS).
Parses `$GPRMC`/`$GPGGA` into points/trajectories carrying time + elevation. CSV
is "tabular points with coordinate columns" and needs X/Y or Lon/Lat column
mapping. Server-side, account for history, 5 GB cap, ZIP/7z batch. Four-step
upload→map→configure→download flow with drag-and-drop.

**Convert.Guru** — Free NMEA → GPX / KML / CSV, or raw-sentence view. Mentions
`$GPGGA` (fix) and `$GPRMC` (transit). Notes that converting to consumer formats
discards low-level satellite/DOP diagnostics. Simple select-file → preview →
convert flow. No documented coordinate-format or column options.

**Bluecover NMEA Analyser** — Decodes GGA, GSA, GSV, RMC, GLL, ZDA (NMEA v4.1).
Extracts latitude, longitude, altitude, time, date, satellite PRNs + SNR, and
HPE/VPE quality. Text-box or file input; shows decoded position/time, a summary,
a parse log, and a map. Exports CSV/KML/GPX. Lets the user enter a reference
lat/lon/alt for comparison.

**Petrichor-Labs `nmea_data_convert`** — Parses RMC, GGA, GLL, VTG, GSA, GSV,
GNS, GST via pynmea2. CSV columns adapt to parsed data; position records carry
timestamp, latitude, longitude, altitude, speed. Groups sentences into "cycles"
(a chosen start sentence, e.g. GNRMC, marks a boundary) and merges same-cycle
sentences into one datetime-stamped record; can backfill missing datetimes.

## Table-stakes → in-model / out-of-model

| capability | competitors | our decision |
|------------|-------------|--------------|
| Parse GGA (time, lat, lon, altitude, fix quality, satellites, HDOP) | all | **in** — primary row source |
| Parse RMC (date, time, lat, lon, speed, course) | all | **in** — supplies the date + speed + course |
| Parse GLL (lat, lon, time) | Bluecover, Petrichor | **in** — fallback position rows |
| Parse VTG (speed, course) | Petrichor | **in** — fills speed/course on the current fix |
| Parse ZDA (date/time) | Bluecover | **in** — supplies date when no RMC |
| Parse GSA (HDOP/PDOP/VDOP) | Bluecover, Petrichor | **in** — HDOP fallback when GGA absent |
| Merge sentences of one cycle into a single timestamped row | Petrichor ("cycles"), MyGeodata | **in** — merge by shared time-of-day; the core value-add |
| Full ISO-8601 timestamp (date + time, not just HH:MM:SS) | MyGeodata, Petrichor | **in** — date propagated from RMC/ZDA |
| Decimal-degrees vs DMS coordinate format | common GIS expectation | **in** — `coordinates` param |
| Altitude unit (m / ft) | GIS tools | **in** — `altitude_unit` param |
| Speed unit (knots / km/h / mph) | VTG carries both knots + km/h | **in** — `speed_unit` param |
| Delimiter choice (comma/semicolon/tab/pipe) | spreadsheet-locale need | **in** — matches sibling `gpx-to-csv` |
| Header row toggle | standard | **in** |
| NMEA `*XX` checksum validation / skip bad lines | robustness | **in** — `validate_checksum` param |
| Fix-quality label (GPS/DGPS/RTK/…) | Bluecover "human understandable" | **in** — mapped to a readable `fix` column |
| Interactive map preview | MyGeodata, Bluecover, Convert.Guru | **out** — needs a map/tile stack; page is a data converter, not a mapper (siblings `gpx-to-geojson`/`photo-gps-mapper` cover mapping) |
| GPX / KML output | Convert.Guru, MyGeodata | **out of THIS tool** — siblings already do `owntracks-to-gpx`, `gpx-to-kml`; keep this one CSV-focused |
| Satellites-in-view detail (GSV PRN/SNR table) | Bluecover, Petrichor | **considered, rejected** — per-satellite SNR is a different (diagnostic) output shape than a track-point CSV; would bloat rows. GGA satellite *count* + HDOP is kept |
| 5 GB / ZIP-batch upload, account history | MyGeodata | **out** — server/account features; gizza is browser-local, no upload |
| Reference-point comparison | Bluecover | **considered, rejected** — analysis feature, out of a converter's scope |

## Design summary

One row per positional fix. Sentences sharing a time-of-day within a cycle are
merged: GGA contributes altitude / fix quality / satellite count / HDOP, RMC
contributes date / speed / course, GLL is a position fallback, VTG fills
speed+course, ZDA/RMC/GSA backfill date and HDOP. Output columns:
`time, latitude, longitude, altitude_*, fix, satellites, hdop, speed_*, course`.
Coordinate format, altitude unit, speed unit, delimiter, header, and checksum
validation are user options. Everything runs in-browser (WASM); nothing is
uploaded — the positioning gap vs. the upload-based competitors.

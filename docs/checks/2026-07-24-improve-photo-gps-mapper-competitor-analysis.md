# photo-gps-mapper — competitor analysis (2026-07-24)

Tool: extract GPS coordinates from a batch of photos and emit a coordinate list
or GeoJSON for mapping. Multi-image input → structured text/JSON output. Pure
Rust (`kamadak-exif`), so it runs on all backends (chat + CLI). Multi-image input
means **no standalone page** (the page file input is a single upload), matching
gif-from-images / duplicate-image-finder / image-collage.

## Competitors skimmed (top 3 reachable)

1. **Coordinates Tools — geojamal** (coordinates.geojamal.com) — batch extract
   lat/lon/altitude from geotagged photos; export CSV, Excel (.xlsx), GPX,
   GeoJSON, KML, TXT. Coordinate-format toggle (DD only / DMS only / both),
   splits results into a "has GPS" table and a "missing GPS" table. 100% offline.
2. **QuickMapTools — Photo Locations** (quickmaptools.com) — extract GPS from
   phone photos, plot on a map, export GeoJSON or CSV. Accepts JPEG/TIFF/PNG/WebP.
   Browser-only, no upload.
3. **Dawarich — Photo Geodata Extraction** (dawarich.app) — extract GPS from EXIF,
   export GPX for GPS devices. Notes only photos with GPS appear; screenshots /
   edited photos may lack location. Browser-only.

(Forensic OSINT image metadata analyzer also seen — GPS + CSV/KML/GeoJSON export,
client-side; used as a cross-check on format coverage.)

## Table-stakes → decision

Per-photo fields:
- filename — **in** (source label, like duplicate-image-finder).
- latitude / longitude (decimal degrees) — **in** (core EXIF decode).
- altitude (m) — **in** (GPSAltitude + GPSAltitudeRef, signed).
- timestamp — **in** (DateTimeOriginal, falls back to DateTime).
- DMS-format coordinates — noted **out of primary path**: GeoJSON/GPX/KML all
  mandate decimal degrees anyway; the `list` format shows DD. DMS is a display
  nicety, not needed for mapping; skipped to keep the descriptor tight.

Export formats:
- GeoJSON (FeatureCollection of Point features) — **in** (default).
- CSV — **in**.
- GPX (waypoints) — **in**.
- KML (Placemarks) — **in**.
- Plain list (`name: lat, lon`) — **in**.
- Excel .xlsx — **out** (CSV imports into any spreadsheet; xlsx is redundant weight).

Options:
- decimal precision — **in** (`precision`, default 6).
- coordinate-format toggle (DD/DMS/both) — **out** (see above).
- valid-only / invalid-only filtering — **partial in**: the report always lists
  which photos lacked GPS (`without_gps`) alongside the mapped output, so users
  see both categories without a filter flag.

Out of model (listed, not built):
- Interactive map rendering / plotting — needs a browser map widget; we emit the
  GeoJSON/KML the user drops into any map viewer instead.
- Reverse geocoding to place names — needs a network geocoder / dataset.
- Excel .xlsx export — redundant with CSV.
- DMS coordinate strings — display nicety; mapping formats use DD.

## Descriptor (in-model set)

- `images` — required source_list (1+), url⊕ref each.
- `format` — enumv `geojson` (default) | `csv` | `gpx` | `kml` | `list`.
- `precision` — integer decimal places for lat/lon, default 6 (0–10).

Output report: `{ format, total, with_gps, without_gps[], output }` where `output`
is the formatted GeoJSON/CSV/GPX/KML/list text. Errors when no photo carries GPS.

NEVER copied competitor copy, branding, or trademarks — all wording above is
paraphrase for design rationale only.

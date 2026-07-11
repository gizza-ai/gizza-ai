# gpx-privacy-scrubber — competitor analysis (2026-07-10)

**Tool:** Strips or fuzzes home/start/end locations and timestamps from a GPX
file before sharing it. Pure-Rust (`quick-xml`), runs on all backends; text-in
(GPX XML) → text-out (scrubbed GPX XML).

## Competitor scan (top 3 real tools)

1. **Strip GPX — Alex Reuneker** (`reuneker.nl/files/gpx/`) — in-browser, nothing
   uploaded. Options: *Remove non-location data* (heart rate, timing, other
   extensions), *Reduce waypoints* by 0–90% (slider), *Remove altitudes* (yes/no).
   No home-location radius, no start/end trim, no coordinate fuzzing.
2. **Sanitizing GPX files — Ole Begemann** (`oleb.net/2020/sanitizing-gpx/`) — a
   shell/XSLT recipe. Removes: all `<extensions>` (speed/course/heart rate),
   `<time>` in metadata **and** every track point, dilution-of-precision
   (`hdop`/`vdop`/`pdop`), the non-standard `type` attribute; neutralises the
   `creator` attribute; strips unused XML namespaces. Keeps coordinates,
   elevation, and track name. Treats timestamps as private. No radius/trim, no
   coordinate rounding.
3. **GPSBabel `radius` filter** (`wiki.openstreetmap.org/wiki/GPSBabel/Using_filters`)
   — `-x radius,distance=1.1K,lat=..,lon=..,exclude` drops all points within a
   circle around a "privacy hot spot" (home/office); chainable for several spots.
   Also a `track,start=YYYYMMDDhh` time filter. CLI only, no browser.

**Adjacent reference — Strava Privacy Zones:** hide the start/end of an activity
within a radius of an address; 5 fixed radii up to 1 mile (⅛, ¼, ⅜, ½, ⅝ mi ≈
200 m … 1 km), or "hide anywhere" which crops the beginning and end of the route.
Applies only to the ends, not mid-route.

## Table-stakes → decisions (all in-model; pure XML rewrite)

| Capability | Competitor(s) | Decision |
|---|---|---|
| Trim/crop points near start & end (home privacy zone) | Strava, GPSBabel `radius` | **`trim_radius_m`** — drop points within N m of the first or last track point (0 = keep whole track) |
| Fuzz exact coordinates | (privacy best-practice; none of the 3 do it directly) | **`precision`** — round lat/lon to N decimal places (5≈1 m … 2≈1.1 km) |
| Strip timestamps (metadata + points) | Ole Begemann, Strip GPX | **`remove_time`** (default on) |
| Strip sensor + accuracy data (HR/cadence/power/temp, hdop/vdop/pdop/sat/fix) | Strip GPX, Ole Begemann | **`remove_extensions`** (default on) |
| Strip elevation | Strip GPX ("remove altitudes") | **`remove_elevation`** (default off — elevation is usually wanted) |
| In-browser, nothing uploaded | Strip GPX | Yes — pure wasm, local only |
| Preset radii chips | Strava's 5 fixed radii | `[[example]]` preset chips (Hide home 100 m / 250 m, Strong fuzz) |

### Out-of-model / deliberately not built
- **Reduce waypoint count by %** (Strip GPX) — a track-simplification concern
  (Douglas–Peucker/decimation), orthogonal to *privacy*; deferred, not a
  privacy gap.
- **Named multi-hotspot radius by address** (GPSBabel chaining, Strava address
  geocoding) — needs geocoding + multi-centre input; the start/end-relative
  `trim_radius_m` covers the common home-at-both-ends case without any lookup.
- **Timestamp shifting/randomising** (GPSBabel Anonymize) — we fully strip time
  instead, which is strictly more private than shifting.

No competitor copy, branding, or trademarks are reproduced; the above is a
paraphrased capability comparison only.

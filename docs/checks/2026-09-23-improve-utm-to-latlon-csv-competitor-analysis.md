# utm-to-latlon-csv — competitor analysis (2026-09-23)

Backlog row: *"Reprojects a CSV of UTM easting/northing/zone coordinates into WGS84 latitude/longitude."*
(type_hint `pure`). Scan run **before** implementing, per `create-next-tool` step 4.

All notes below are paraphrased observations of publicly visible behaviour. No competitor copy,
wording, branding or trademark is reproduced or reused anywhere in this tool.

## Dup check (why this is a new block, not a near-dup)

`ls blocks/ | grep -iE 'utm|latlon|coord|geo|gps|proj'` plus a source grep for
`easting|northing|transverse mercator|wgs84`:

- `blocks/utm-link-builder` — unrelated homonym: **U**rchin **T**racking **M**odule campaign query
  params (`utm_source`, `utm_medium`, …), confirmed in its `core/src/lib.rs` (`UTM_KEYS`).
- `blocks/cartesian-to-polar-csv` — planar x/y ↔ r/θ; its own copy states it does **not** do
  geodetic coordinate systems. It only borrows `easting`/`northing` as *aliases* for x/y.
- `blocks/shapefile-to-geojson` — explicitly **warns** when a `.prj` is a projected CRS
  (test `projected_prj_warns_that_coordinates_are_not_lon_lat`, e.g. `NAD83 / UTM zone 10N`) and
  tells the user output is not EPSG:4326. It reads a CRS name; it never reprojects.
- `blocks/csv-to-geojson`, `blocks/geojson-*`, `blocks/gpx-*`, `blocks/nmea-to-csv`,
  `blocks/geofence-check`, `blocks/geo-cluster` — all consume coordinates that are **already**
  lon/lat; none reprojects.
- `docs/tool-skiplist.txt` line 583 (`geocode`, skiplisted) states outright that
  "Coordinate-format conversion (decimal degrees ↔ DMS ↔ UTM) would be a DIFFERENT, buildable
  tool" — i.e. this row is the buildable one that note points at.

Conclusion: no reprojection engine exists in the repo. Build it.

## Competitors skimmed

1. **Engineering Toolbox — UTM ↔ latitude/longitude converter**
   (`engineeringtoolbox.com/utm-latitude-longitude-d_1370.html`)
   Single-point fields for easting, northing and zone number, plus a textarea for a comma-separated
   list with one point per line. Output offered as decimal degrees *or* degrees/minutes/seconds.
   Uses the WGS84 ellipsoid and notes ETRS89 can differ by over half a metre. Notable weaknesses:
   it is **northern-hemisphere only**, it requires northing **before** easting in the batch textarea
   with an explicit "check the sequence of input values" warning, and the first textarea line must
   be left untouched. Controls: text fields, textarea, one convert button.

2. **GlandNav UTM → lat/lng converter** (`glandnav.com/tools/utm-to-latlng-converter`)
   All 60 zones and both hemispheres; accepts UTM, MGRS and lat/lon input with format
   auto-detection; WGS84 only. Bulk mode via CSV/TXT upload. Exports CSV, GeoJSON, KML and
   clipboard copy; decimal degrees and DMS output. Claims centimetre-level results and processes
   >10,000 coordinates in chunks. Controls: format-detecting input box, map click, file upload,
   export buttons.

3. **Earth Point — Batch Convert** (`earthpoint.us/BatchConvert.aspx`)
   Spreadsheet (xls/xlsx/xlsm) or text/CSV upload. Recognises the input by **header names**, one of
   five accepted heading sets — for UTM it wants `Zone`, `Easting`, `Northing` (optional
   `UnitOfMeasure`); extra columns are ignored. Zone values carry a band letter (`10S`). Target
   system is a dropdown (lat/lon decimal degrees, decimal minutes, decimal seconds, MGRS, UTM,
   State Plane, GARS, Maidenhead, GeoRef, Plus Code, what3words, Township & Range, …), with a second
   dropdown choosing whether converted columns land First / After input coordinates / Last.
   Hard gate: **only the first five rows convert** unless you sign in or subscribe.

(A fourth data point for algorithm conventions, not a UI competitor: the widely used Python `utm`
package — `from_latlon` / `to_latlon`, `force_zone_number`, `northern` flag, and *no* Norway/Svalbard
zone exceptions.)

## Table stakes → where each one lands

Every item is either a descriptor param (in-model) or an explicitly listed out-of-model gap.

| Table stake | Seen in | Decision |
| --- | --- | --- |
| Easting / northing / zone as **named columns**, extra columns ignored or carried | Earth Point, Esri thread | **in-model** → `easting_column`, `northing_column`, `zone_column` (header name *or* 1-based index, auto-detected when blank) + `keep_columns` |
| Whole-CSV batch, one point per line | all three | **in-model** → `csv` param, 5 MB / 200,000-row caps |
| Zone stated **once** for the whole file (no per-row zone column) | Earth Point (`ToUtmZone`), Esri thread | **in-model** → `zone` fallback param |
| Zone **band letter** (`33U`, `10S`) accepted | Earth Point, GlandNav | **in-model** → zone parser accepts `33`, `33U`, `33 N`, `33n`, `EPSG:32633`, `32733` |
| **Both hemispheres** | GlandNav (Engineering Toolbox fails this) | **in-model** → `hemisphere` = auto \| north \| south, auto derived from band letter / EPSG code |
| Decimal degrees **and** DMS output | Engineering Toolbox, GlandNav | **in-model** → `coord_format` = decimal \| dms \| ddm (Earth Point's decimal-minutes form too) |
| Output precision control | implied by "centimetre level" claims | **in-model** → `decimals`, 0–12, default 6 (≈0.1 m of latitude) |
| Export as **CSV / GeoJSON / KML** | GlandNav | **in-model** → `output` = csv \| tsv \| json \| table \| geojson \| kml |
| Reverse direction (lat/lon → UTM) | Engineering Toolbox ("and back"), GlandNav, Earth Point | **in-model** → `direction` = utm_to_latlon \| latlon_to_utm, with the zone auto-picked from longitude (incl. the Norway 32V and Svalbard 31X/33X/35X/37X exceptions) unless `zone` forces one |
| Datum / ellipsoid choice | Engineering Toolbox mentions WGS84 vs ETRS89 | **in-model, scoped** → `ellipsoid` = wgs84 \| grs80 \| clarke1866 \| international1924 selects the ellipsoid of the inverse/forward Transverse Mercator. A datum **shift** is explicitly *not* applied — see out-of-model below |
| Swapped easting/northing is the #1 user error | Engineering Toolbox's own warning | **in-model, improved** → `validate_ranges` bounds-checks easting/northing/zone and names the offending row with a swap hint, instead of silently returning a wrong point |
| Delimiter other than comma (`;` locales, TSV) | generic CSV tooling | **in-model** → `delimiter` = auto \| comma \| semicolon \| tab \| pipe |
| Headerless numeric rows | Engineering Toolbox textarea | **in-model** → `has_header` |
| Preset examples to click | GlandNav/Earth Point ship samples | **in-model** → `[[example]]` chips on the page |

### UX control patterns matched

- Earth Point's target-system dropdown → our `coord_format` + `output` `<select>`s with friendly
  `[input.labels]` (values stay canonical).
- Precision as a drag control → `decimals` uses `kind = "slider"` (0–12, step 1), per
  `references/page-patterns.md`.
- Sample/preset buttons → five `[[example]]` preset chips (NYC zone 18N, southern-hemisphere
  Sydney 56S, one-zone-for-the-whole-file, DMS output, reverse direction).
- Copy/download of the result → the generator gives `format = "text"` pages a download link for
  free; no per-tool work needed.

### Out-of-model / deliberately not built (listed, not dropped)

- **Datum transformation** (NAD27 → WGS84, ETRS89 → WGS84, etc.). Changing the *ellipsoid* is
  closed-form and shipped; shifting between *datums* needs published 7-parameter Helmert values or
  a NADCON/NTv2 grid shift file per region — external data, not embeddable. Stated plainly in the
  page copy so nobody mistakes `ellipsoid = clarke1866` for a NAD27→WGS84 conversion.
- **MGRS / military grid reference input** (GlandNav). A different input grammar (grid-square
  letters), deserving its own block; this tool takes numeric easting/northing + zone.
- **State Plane, GARS, GEOREF, Maidenhead, Plus Code, what3words, Township & Range, Texas
  abstracts** (Earth Point). Each needs its own zone-definition table; out of scope for a UTM tool.
- **UPS / polar stereographic** for latitudes beyond ±80/84°, where UTM is undefined. We error with
  the actual latitude instead of extrapolating.
- **Interactive map click / map preview** (GlandNav). The page surface is a form; no map canvas.
- **Spreadsheet (.xlsx) upload** (Earth Point). `blocks/xlsx-to-csv` already covers that step;
  chaining it is the intended path rather than duplicating a decoder here.
- **Feet / US-survey-feet input units** (Earth Point's `UnitOfMeasure`). Metres only; a unit
  conversion belongs upstream. Noted in the page limits section.
- **Clipboard/file-picker upload** — pure tools take pasted text on the page; the CLI reads a value
  from argv, and `xargs`/shell redirection covers files.

### Things we do that none of the three do

- No row cap behind a sign-in (Earth Point stops at 5 rows); 200,000 rows locally.
- Runs entirely in the browser tab / CLI — coordinates never leave the machine.
- Bounds validation that *names the row* and suggests the easting/northing swap, rather than the
  competitor pattern of a prose warning above the textarea.
- Round-trip-checked engine: the forward and inverse Transverse Mercator series are unit-tested
  against each other and against published reference points.

## Verification plan (advertised-values matrix)

One real run per enum choice and accepted value form:
`direction` ×2, `coord_format` ×3, `ellipsoid` ×4, `hemisphere` ×3, `delimiter` ×5, `output` ×6,
zone spellings (`18`, `18N`, `56S`, `EPSG:32756`, `32718`), `decimals` at 0 and at the 12 cap,
`has_header` off (non-default checkbox), `keep_columns` off, `validate_ranges` off, the exact
5 MB / 200,000-row cap boundary, and the swapped-column error path. Page spec asserts exact output
text plus one `?param=` deep link; the page's generated CLI example is copy-paste-run verbatim.

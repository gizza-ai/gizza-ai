# kml-to-geojson — competitor scan + design decisions (2026-08-15)

Scan run before the build. Every note below is **paraphrased** from public
product/documentation pages; no competitor copy, branding, trademarks or assets
are reproduced in the tool, its page, or this file.

## Duplicate check

`ls blocks/ | grep -iE 'geo|kml|kmz|gpx|map'` surfaced three neighbours. Each was
read (`core/src/lib.rs`) before deciding:

- **`gpx-to-geojson`** — a GPS-track tool that *also* accepts KML on its way in:
  its `kml_to_geojson()` already maps Placemark geometry, ExtendedData,
  TimeSpan/TimeStamp and Style/StyleMap into a GeoJSON FeatureCollection. Its
  *reverse* direction is GeoJSON → **GPX**; it cannot emit KML, cannot read KMZ,
  has no folder handling and no coordinate-precision control.
- **`gpx-to-kml`** — writes KML, but only from **GPX** input. It has no GeoJSON
  reader at all.
- **`geojson-format` / `geojson-to-csv` / `topojson-to-geojson` /
  `shapefile-to-geojson`** — other legs of the converter family; none touch KML.

Verdict: **not a duplicate, built.** The row's deliverable is KML/KMZ ↔ GeoJSON.
Two of its three halves exist nowhere in the repo — **KMZ (zipped KML) input**
and **GeoJSON → KML output** — and the forward half is deliberately *not*
re-implemented: this block takes `gizza-ai-gpx-to-geojson-core` as a path
dependency and reuses its proven Placemark parser, then layers the KML-specific
work on top (KMZ unzip, folder paths, coordinate precision, the KML writer). So
the repo gains capability without gaining a second copy of the parser.

## Competitors reviewed (4 + the search-result field)

One `WebSearch` for the tool's function returned ten candidate converters; the
four below were reachable tool pages (not listicles or login walls) and were
fetched directly.

1. **QuickMapTools — KML to GeoJSON.** Browser-local, no upload. Drag-and-drop or
   click-to-browse, an interactive map preview of the result, and placemark
   name/description carried into feature properties. Notably it maps **KML
   folders onto a `folder` feature property** — the same shape adopted below.
   States plainly that **styling is dropped**, and its KMZ story is a manual
   workaround (rename to `.zip`, extract the `.kml` yourself). Batch conversion
   is a paid tier.
2. **MapKmlTools — KML to GeoJSON.** Reads `.kml` **and `.kmz`** (extracts the
   embedded KML automatically). Advertises all standard geometries including
   MultiGeometry and nested folders, ExtendedData attributes preserved as
   properties, basic stroke/fill/width styling preserved as properties, and
   RFC 7946-compliant output. Download the `.geojson` or copy the raw JSON; map
   preview alongside. Publishes a 50 MB input cap. FAQ covers privacy, attribute
   preservation, KMZ, styling, size, geometry coverage, and consuming the result
   in common web-map libraries.
3. **GeoUtil — KML to GeoJSON.** Client-side, and the only one of the four that
   also offers a **KML → KMZ** direction. Documents what it will *not* carry
   over — NetworkLinks/external references, screen overlays, tours, custom icon
   files (URL kept, asset not embedded) — and notes altitude modes may be
   flattened. Confirms both formats are WGS84 so no reprojection is involved.
   No GeoJSON → KML direction.
4. **MyGeodata Cloud — KMZ to GeoJSON.** The server-side end of the market: a
   four-step upload → review-on-map → configure → download-zip workflow, batch
   uploads via ZIP/7z, 7,000+ coordinate systems to reproject into, an API, and
   a 5 GB cap. Free tier is credit-limited; data is stored server-side and
   deleted on a timer.

Also seen in the result list but not fetched (aggregator/multi-format
front-ends of the same shape): Atlas, iGISMap, NeatoGeo, kmz2shp, KMLConverter —
the last of which is the one other place a KML↔GeoJSON *round trip* is offered.

## Table stakes → where each one landed

| Table stake | Seen at | In / out of model | Landed as |
| --- | --- | --- | --- |
| KML → GeoJSON FeatureCollection | all 4 | in | default direction (`output_format=geojson`) |
| KMZ (zipped KML) input | MapKmlTools, GeoUtil, MyGeodata | in, with a caveat | auto-detected **base64** KMZ in `input`; the entry is picked as `doc.kml` → first `*.kml`. A raw binary drop is not possible in a text field — the page says so and shows the one-liner to produce the base64 |
| Point / LineString / Polygon / MultiGeometry | all 4 | in | inherited from the reused parser (MultiGeometry → GeometryCollection) |
| name / description → properties | all 4 | in | inherited |
| ExtendedData → properties | MapKmlTools | in | inherited |
| Styles preserved as properties | MapKmlTools (QuickMapTools drops them) | in | `include_styles` (default on) → simplestyle-spec `stroke`, `stroke-width`, `stroke-opacity`, `fill`, `fill-opacity`, `marker-color` |
| Folder hierarchy → a `folder` property | QuickMapTools, MapKmlTools | in | `include_folders` (default on) → `folder` property carrying the `Document/Folder/Sub` path |
| Coordinate precision / smaller output | implied by MapKmlTools' "smaller, faster-parsing" angle | in | `precision` (0–15, default 6 ≈ 0.1 m) |
| Reverse GeoJSON → KML | KMLConverter (round trip); GeoUtil only does KML → KMZ | in | `output_format=kml`, with `document_name`, `altitude_mode`, and simplestyle → `LineStyle`/`PolyStyle`/`IconStyle` when `include_styles` is on; `folder` property regroups into `<Folder>` when `include_folders` is on |
| Copy result / download the file | MapKmlTools, QuickMapTools | in | platform: every `format = "text"` page ships Copy + Download |
| Local, no upload, no account | QuickMapTools, MapKmlTools, GeoUtil | in | inherent — wasm in the page, wasm in the CLI |
| Stated limits + what is dropped | GeoUtil, MapKmlTools | in | page copy: 2 MB input cap, WGS84 only, and the explicit not-carried list |
| Interactive map preview of the result | all 4 | **out** | needs a map library + remote tiles; the page is offline wasm with no network |
| CRS / reprojection to other coordinate systems | MyGeodata | **out** | needs a projection database; KML is WGS84-only by spec, so the conversion itself never needs it |
| Batch / multi-file / ZIP upload | QuickMapTools (paid), MyGeodata | **out** | the page is a single text input; per-file runs via the CLI cover the scripted case |
| NetworkLink / external KML resolution | called out by GeoUtil | **out** | fetching a remote href is network I/O; blocks are offline |
| Embedded KMZ assets (icons, overlays, models) | GeoUtil, MyGeodata | **out** | binary side-car assets have nowhere to go in a GeoJSON text output; the icon href is kept as a property |
| Shapefile / CSV / GPX targets | iGISMap, NeatoGeo, kmz2shp | **out of scope** | already separate blocks (`shapefile-to-geojson`, `geojson-to-csv`, `gpx-to-geojson`) |
| API access, accounts, paid tiers | MyGeodata | **out** | no server, no accounts |
| 50 MB–5 GB inputs | MapKmlTools, MyGeodata | **out** | the wasm sandbox is 64 MiB; a 2 MB cap is enforced with an actionable error |

## Considered, rejected

- **KMZ *output*.** GeoUtil ships KML → KMZ. A zip is binary, and this page's
  output is text — it would have to come back as base64 the user then decodes,
  which is worse than handing them the `.kml` the page already downloads (any
  zip tool makes a KMZ from it). Stated on the page instead of built.
- **A `styles` → separate style-table output mode.** Only useful for a map
  renderer that reads its own style schema; simplestyle-spec is the interchange
  every consumer named by the competitors already understands.

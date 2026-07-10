# metadata-privacy-linter — competitor analysis (2026-07-10)

Scan done BEFORE implementing, per the create-next-tool recipe. All notes are
**paraphrased**; no competitor copy, branding, or trademarks are reproduced.
Goal: identify table-stakes params/defaults/UX and tag each **in-model** (browser-local,
pure-Rust/wasm, no server/account) or **out-of-model**.

## Tool being built

`metadata-privacy-linter` — "Scans an image's EXIF, XMP, and IPTC for privacy-sensitive
fields (GPS, device serials, owner name, software) and lists what would leak if shared."
Type: **pure** (image bytes in → text/JSON report out). Surfaces: **chat + CLI only**.
No standalone page — the pure page driver only marshals text/number field values into the
wasm export; it has no path to hand image *bytes* to a pure `run()`, so image-file→text
tools have never shipped a page (precedent: `image-metadata-viewer`, `image-info`,
`detect-file-type`, `strip-exif`, `image-color-picker`). Documented here rather than shipping
a non-functional page.

## Competitors surveyed (paraphrased)

1. **A privacy-oriented metadata analyzer.** Reads EXIF, XMP, IPTC, ICC, and JFIF. Flags GPS
   location and device info, decodes exact GPS coordinates, and (server-side) reverse-geocodes
   to a street address shown on an interactive map. Parsing runs client-side.
2. **A drag-and-drop EXIF/IPTC/XMP viewer.** Accepts JPEG/PNG/WebP; shows a visual camera-settings
   summary, a searchable field table, and a map when GPS is present. Client-side only.
3. **A forensic OSINT metadata reader.** EXIF, GPS, XMP edit history, IPTC copyright, and maker
   notes from JPEG/HEIC/TIFF/RAW. Duplicate detection, side-by-side compare, export to
   CSV/KML/GeoJSON/PDF.
4. **A general online EXIF/XMP/IPTC/ICC viewer.** Surfaces the full field list and a GPS map link
   (OpenStreetMap) when coordinates look valid. File analyzed locally, not uploaded.
5. **A photo metadata remover / privacy stripper.** Lists every EXIF/IPTC/XMP field the file
   carries, then one-click removes GPS, camera data, timestamps, and author fields. Runs in the
   browser.

Cross-cutting theme in the space: a camera **serial number** is a device fingerprint that links
otherwise-anonymous photos to one owner; **GPS** reveals home/routine; **timestamps** reveal
patterns. Most large social platforms strip EXIF on upload, but some chat apps (e.g. Discord,
iMessage) do not — so "it's fine, the platform strips it" is not reliable.

## Table-stakes → tag

| Capability | In / Out | Decision |
|---|---|---|
| Read **EXIF/TIFF** privacy fields (GPS, serials, owner, software, timestamps, description) | in-model | **Built** — `kamadak-exif`, classified by category + risk |
| Read **XMP** privacy fields (dc:creator, rights, CreatorTool, aux serials, exif GPS, keywords) | in-model | **Built** — locate the XMP packet in the bytes, scan for known property local-names |
| Read **IPTC (IIM)** privacy fields (By-line, Copyright, City/State/Country, Credit, Caption, Keywords) | in-model | **Built** — parse the JPEG APP13 Photoshop IRB, resource 0x0404, IIM datasets |
| Decode **GPS to decimal lat/long** | in-model | **Built** — DMS→decimal, plus an OpenStreetMap URL string |
| Classify each finding by **category + risk** and give a "what would leak" summary | in-model | **Built** — this is the differentiator vs. a raw viewer |
| **Redact values** so the report itself is shareable | in-model | **Built** — `reveal_values` boolean (default true) |
| **Risk filter** (only show medium+/high) | in-model | **Built** — `min_risk` enumv (all/medium/high, default all) |
| Client-side / never-uploaded processing | in-model | **Matches by design** — browser-local wasm, no account/server |
| Platform-stripping guidance (which apps keep metadata) | in-model (copy) | **Built** — FAQ + limits notes |
| Reverse-geocode GPS to a **street address** | out-of-model | Listed — needs a geocoding backend/API; we emit coordinates + an OSM link only |
| Interactive map / searchable table UI | out-of-model | Listed — no page (see above); chat/CLI render text |
| AI-origin detection, steganography, ELA forensics | out-of-model | Listed — needs an ML model; outside gizza's pure-Rust+ffmpeg model |
| Duplicate detection / side-by-side compare | out-of-model here | Listed — a separate concern (`duplicate-image-finder` already exists) |
| Export to CSV/KML/GeoJSON/PDF | out-of-model here | Listed — the JSON report is machine-readable; extra export formats are scope creep for a linter |
| One-click **removal/strip** of metadata | out-of-model here | Listed — that is `strip-exif`'s job; this tool *reports*, it does not mutate |

## Design decisions

- **Descriptor params:** `min_risk` (enumv: `all`|`medium`|`high`, default `all`) and
  `reveal_values` (boolean, default `true`). Both `.describe()`d. Keeps the schema tiny while
  covering the two in-model UX knobs competitors expose (severity filter, shareable/redacted view).
- **Output:** a flat JSON report — `format`, `clean` (bool), `findings_count`, `findings[]`
  (`source`/`field`/`category`/`risk`/`value`), distinct `categories`, decoded `gps` + `gps_map_url`,
  and a plain-English `summary` ("Would leak: GPS location, camera serial number, …"). An LLM/CLI
  user reads it directly.
- **Not built (honest):** reverse geocoding, ML forensics, map UI, extra export formats, and
  metadata removal — each is out-of-model or already covered by a sibling tool.

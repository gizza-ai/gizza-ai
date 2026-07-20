# exif-edit — competitor analysis (2026-07-20, built-in improve pass)

Backlog row: `exif-edit` — "Write, edit, or selectively strip individual EXIF/IPTC/XMP fields
(date, GPS, copyright) on a photo." Type: pure (img-parts + kamadak-exif; no ffmpeg).

Related existing blocks (NOT dups — confirmed by reading their cores):
- `strip-exif` — removes ALL metadata wholesale; no field-level writes.
- `image-metadata-viewer` — read-only EXIF dump.
- `metadata-privacy-linter` — read-only risk report.
`exif-edit` is the missing WRITE side: set/replace individual fields, remove selected groups,
keep everything else, never re-encode pixels.

## Competitors reviewed (paraphrased; no copy/branding taken)

1. **theXifer.net** — web EXIF/IPTC/XMP editor. ~175 editable tags across EXIF/IPTC/XMP;
   GPS via an interactive map with draggable marker + place search; date picker; up to 30
   saved metadata presets; metadata copy/paste between files; batch (5 files/60 MB free,
   100 files/300 MB paid); inputs JPG/JFIF/TIF/PNG/GIF/WebP + video (MP4/MOV/3GP) + PDF;
   cloud import (Drive/Dropbox/Flickr).
2. **EXIFEditor.io** — browser-local editor (no upload). Edits GPS coordinates + facing
   direction, timestamps, camera make/model, software/app used; per-tag removal of private
   tags; map slider UI; full tag reference table.
3. **EXIFDataView.com /edit-exif** — browser-local editor. Edits GPS lat/lon, capture
   date/time, camera make/model + lens info, author + copyright; JPEG/PNG/TIFF; stresses
   pixels/quality untouched.

## Table stakes → decision

| Capability | Competitors | Tag | Decision |
|---|---|---|---|
| Set GPS latitude/longitude (decimal degrees) | all 3 | in-model | `latitude` + `longitude` params (must come together), −90..90 / −180..180, written as GPS IFD rationals + N/S / E/W refs + GPSVersionID |
| Set GPS altitude | theXifer, EXIFDataView | in-model | `altitude` param (meters; negative = below sea level → GPSAltitudeRef 1) |
| Change date taken | all 3 | in-model | `date_taken` param; accepts `YYYY-MM-DD HH:MM:SS`, EXIF `YYYY:MM:DD HH:MM:SS`, ISO `T` form, or date-only; writes DateTimeOriginal + DateTimeDigitized + DateTime |
| Author/artist + copyright | all 3 | in-model | `artist`, `copyright` params (TIFF Artist/Copyright) |
| Camera make/model | all 3 | in-model | `make`, `model` params |
| Software/creator app | EXIFEditor.io | in-model | `software` param |
| Image description/caption | theXifer (tag set) | in-model | `description` param (ImageDescription) |
| Remove individual/private tags (e.g. GPS only) | all 3 | in-model | `remove` param: comma list of groups `gps, date, artist, copyright, description, software, camera, serials, xmp, iptc` |
| Strip whole XMP / IPTC blocks | theXifer (XMP/IPTC editing) | in-model (strip), out-of-model (field-level edit) | `remove=xmp` / `remove=iptc` drop the APP1-XMP / APP13 segments (PNG: the XMP iTXt chunk). Field-level XMP/IPTC *editing* (XML packet rewriting) is out-of-model for now |
| Pixels never re-encoded | all 3 | in-model | img-parts segment splice, byte-identical compressed data |
| JPEG input | all 3 | in-model | supported |
| PNG input | theXifer, EXIFDataView | in-model | supported (eXIf chunk) |
| TIFF / WebP / HEIC / GIF / video / PDF | theXifer (TIFF/WebP/GIF/video/PDF), EXIFDataView (TIFF) | out-of-model | not supported; stated in the tool description + FAQ. TIFF = EXIF *is* the container (rewrite risk), HEIC = ISO-BMFF boxes, WebP EXIF flagging in img-parts unproven |
| Map picker for GPS | theXifer, EXIFEditor.io | out-of-model | no page surface (pure image-bytes-out tools are chat+CLI only in this repo); decimal-degree params are the CLI/LLM-native equivalent |
| Batch / multi-file, cloud import | theXifer | out-of-model | single-source tool shape |
| Saved metadata presets/templates, metadata copy between files | theXifer | out-of-model | chat can replay parameters; no persistent presets |
| GPS facing direction (GPSImgDirection) | EXIFEditor.io | out-of-model (deferred) | niche; not in the first descriptor — noted here so it is not silently dropped |
| Per-tag arbitrary editing (175 tags) | theXifer | out-of-model | curated field set instead; arbitrary tag names would be schema-unfriendly for LLM/CLI |

## Design notes / policies

- **Surfaces:** chat + CLI, **no page** — image-bytes output has no page render mode and the
  pure-wasm page runtime has no file input (both are ffmpeg-runtime patterns). Precedent:
  strip-exif, rotate-image, flip-image, image-collage.
- **Rewrite policy:** the EXIF segment is *rebuilt* (kamadak-exif experimental Writer) with all
  existing fields carried over except: edited fields (replaced), removed groups, the embedded
  thumbnail IFD (offset-bearing; also a privacy leak — reported), MakerNote (opaque
  maker-specific blob with internal absolute offsets that break on any rewrite — dropped and
  reported rather than silently corrupted), and offset/pointer tags (recomputed by the writer).
- **Conflicts:** setting a field AND removing its group in one call errors; at least one edit
  (set or remove) is required; `latitude`/`longitude` must be provided together.
- **Existing malformed EXIF** errors (with a pointer at strip-exif) instead of silently
  discarding what the parser could not read.
- 16 MiB input cap (strip-exif parity), same-format output, filename suffixed `-edited`.

## Verification plan (no page → no Playwright spec)

- Unit: happy paths (JPEG fresh EXIF, JPEG existing EXIF preserve+replace, PNG eXIf, every
  remove group, GPS sexagesimal conversion incl. carry + sign) + errors (bad date, lat without
  lon, unknown remove entry, set+remove conflict, no-op call, non-JPEG/PNG input, malformed EXIF).
- Schema drift-guard test against the authored chat schema.
- CLI: real-URL run against a public JPEG with GPS EXIF (ianare/exif-samples via
  raw.githubusercontent.com) — set + remove cases, exact summary-output case, cap behavior,
  PNG secondary-format case, graceful fetch error on the generated example URL.

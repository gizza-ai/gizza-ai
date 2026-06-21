# image-metadata-viewer — competitor analysis & differentiation

**Tool:** `gizza-ai/image-metadata-viewer` — read and display EXIF/IPTC metadata
(camera, GPS, timestamps) from an image.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `exiftool` | CLI | The gold standard, but a heavyweight Perl install; overkill for a quick look. |
| Online EXIF viewers (exifdata, metapicz, …) | Web | **Upload your photo to a server** — and photo EXIF often contains your home GPS, so that's a real privacy leak. Ad-heavy. |
| OS "Get Info" / Properties | App | Shows a handful of fields; no full tag dump, no decimal GPS. |
| `identify -verbose` (ImageMagick) | CLI | Verbose but noisy; not structured. |

## How gizza's tool is better / different

1. **Local — the photo never leaves the device.** Runs in WASM (chat SW + CLI).
   This matters most here precisely because EXIF can carry **GPS home/work
   locations** — the online viewers are a privacy hazard.
2. **Full structured dump.** Every tag with its value and IFD, as clean JSON —
   not a curated subset.
3. **GPS decoded for you.** The GPS lat/lon rationals + N/S/E/W refs are converted
   to **signed decimal degrees** (drop straight into a map), in addition to the
   raw DMS fields.
4. **Broad format support** via `kamadak-exif` (JPEG/TIFF/HEIF/PNG/WebP).
5. **Two surfaces, one Rust core.** Chat ("what camera/where was this taken?")
   and CLI.

## Verification

Core unit tests parse a hand-assembled EXIF JPEG (Make field) and error on
garbage. **End-to-end CLI** on a real Nikon sample photo read **61 fields** —
Make=NIKON, Model=COOLPIX P6000, DateTime, ExposureTime 1/75 s, FNumber f/5.9 —
and decoded **GPS to 43.467448, 11.885127** from the DMS rationals.

## Surfaces & honest scope

- **Chat + CLI only — no web page** (image file input + text/JSON output; the
  no-page file-input pattern, like `image-info` / `detect-file-type`).
- Reads EXIF/TIFF (the dominant camera metadata). XMP/IPTC blocks aren't
  separately parsed — a possible future addition.

## Possible future enhancements

- Parse XMP / IPTC segments too.
- A "privacy" mode that flags GPS/serial-number fields.
- Emit a map link for the GPS coordinate.

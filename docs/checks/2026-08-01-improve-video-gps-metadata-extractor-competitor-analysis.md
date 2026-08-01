# Competitor analysis — video-gps-metadata-extractor (2026-08-01)

Tool function: extract static GPS/location metadata embedded in MP4/MOV/QuickTime
video containers, especially the QuickTime `©xyz` atom and Apple's ISO 6709
location key, and report latitude/longitude without uploading the video.

## Landscape

### 1. ExifTool / desktop metadata viewers
- ExifTool is the reference command-line utility for this class of metadata. It
  exposes GPS latitude/longitude and QuickTime location fields, and supports raw
  decimal output for scripting.
- General desktop metadata viewers such as MediaInfo also surface container-level
  metadata, but typically emphasise codec/container details rather than a focused
  "is this video geotagged?" workflow.

### 2. ffprobe / FFmpeg metadata inspection
- ffprobe can print format-level metadata as JSON and exposes `location` /
  language-suffixed location tags when present in MOV/MP4 files.
- Its output is broad and low-level; users must already know which metadata keys
  to search for and how to parse ISO 6709 strings.

### 3. Online video metadata extractors and privacy checkers
- Web tools generally ask users to upload a full video, then display a broad
  metadata report. The table-stakes are a simple upload/paste flow, clear
  latitude/longitude display, JSON/scriptable output, and privacy-focused copy.
- For this repo, upload/server processing is out-of-model; the in-model equivalent
  is an in-browser WASM parser that accepts file bytes as base64/hex and never
  sends them to a server.

## Table-stakes → decisions

| Capability | Seen in | In/out of model | Decision |
| --- | --- | --- | --- |
| Read QuickTime `©xyz` user-data atom | ExifTool, ffprobe, iOS/Android geotag docs | in-model | Parse the MP4/MOV box tree directly and decode the QuickTime text atom |
| Read Apple `com.apple.quicktime.location.ISO6709` metadata key | ExifTool / QuickTime metadata docs | in-model | Parse `meta`/`keys`/`ilst` + `data` boxes and map item index to key name |
| Parse ISO 6709 to decimal latitude/longitude | ExifTool `-n`, ffprobe users | in-model | Return signed decimal lat/lon and altitude when present |
| Human report and machine-readable output | ExifTool/ffprobe JSON conventions | in-model | `output=report|json` |
| Privacy-first local processing | Online privacy checkers | in-model | Browser WASM parser; no upload/server requirement |
| Accept whole files directly in the page | Online upload tools | partially in-model | Current generic page uses text fields, so accept base64/hex bytes; docs show taking only the metadata head |
| Codec/duration/stream metadata | MediaInfo, ffprobe | out-of-scope | Existing `media-info` covers general media metadata; this tool stays GPS-focused |
| Per-frame GPS telemetry (GoPro GPMF, dashcam tracks) | specialised telemetry tools | out-of-model | Listed as a limit; requires proprietary binary stream parsing beyond static QuickTime tags |
| Removing GPS metadata | ExifTool, metadata strippers | existing sibling tools | Existing `video-strip-metadata` / privacy tools handle removal; this tool only detects/reports |

## Existing-block duplicate check

- `media-info` reports container/track codec metadata, not static QuickTime GPS
  location tags.
- `metadata-privacy-linter` and `gps-location-remover` are image-oriented.
- `video-strip-metadata` removes metadata via ffmpeg but does not extract or parse
  latitude/longitude.

So this tool is not a semantic duplicate: it fills the focused "does this video
leak a location?" inspection path.

> Original work only — competitor behaviour is paraphrased; no competitor copy,
> branding, or trademarks are reused.

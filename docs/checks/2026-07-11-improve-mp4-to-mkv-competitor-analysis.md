# mp4-to-mkv — competitor analysis (2026-07-11)

Tool: `mp4-to-mkv` — remux an MP4 into a Matroska (`.mkv`) container **without
re-encoding** (`-i in.mp4 -map 0 -c copy out.mkv`), preserving every video,
audio, subtitle and data stream so soft subtitles / extra audio tracks can be
added later. Runs entirely in-browser via ffmpeg-wasm; nothing is uploaded.

Research was done with live web search (queries + sources below). Competitor
names and stated capabilities are taken from those pages, not fabricated.

## Competitors surveyed

1. **CloudConvert — MP4 to MKV** (cloudconvert.com/mp4-to-mkv). Server-side.
   Lets you control resolution, quality and file size; integrates Google
   Drive / Dropbox / OneDrive. Free tier capped at 25 conversions/day. Files
   are uploaded to their servers.
2. **Convertio — MP4 to MKV** (convertio.co/mp4-mkv). Server-side. Exposes
   codec, resolution, bitrate and audio-channel controls; batch/multi-file;
   Google Drive / Dropbox import. Uploads required.
3. **FreeConvert — MP4 to MKV** (freeconvert.com/mp4-to-mkv). Server-side,
   any browser. Batch convert, "Advanced Settings" to tune parameters,
   256-bit SSL, files auto-deleted after a few hours. Uploads required.
4. **Flonnect — Remux container** (flonnect.com/media-tools/remux-container).
   Browser-local (WebAssembly), lossless remux, no uploads, no sign-up —
   the closest match to gizza's model.
5. **CutFast — MP4 to MKV** (cutfa.st/features/mp4-to-mkv). Browser-local;
   remuxes when possible, transcodes when needed; no upload.

## Table-stakes (what a MP4→MKV tool is expected to do)

- **Lossless remux by default** (`-c copy`) — the headline of every remux tool;
  no quality change, finishes in seconds. ✅ our default and only mode.
- **Preserve all streams** — video + all audio + subtitles + data. MKV is a
  superset container, so `-map 0` keeps everything MP4 held. ✅ (`-map 0`).
- **Private / no upload** — the browser-local competitors (Flonnect, CutFast)
  lead on this; the server tools (CloudConvert/Convertio/FreeConvert) upload.
  ✅ gizza runs ffmpeg-wasm locally, nothing leaves the device.
- **Accept the common input** — MP4 (and the MP4-family: `.m4v`, and other
  ISO-BMFF). ✅ input is `video/*`, output is always `.mkv`.

## Gap analysis (fit to gizza's model)

| Feature | Competitors | gizza mp4-to-mkv | Decision |
|---|---|---|---|
| Lossless `-c copy` remux | all remux tools | ✅ default & only path | met |
| Preserve every stream (`-map 0`) | implicit | ✅ explicit `-map 0` | met |
| Browser-local / no upload | Flonnect, CutFast | ✅ | met (parity w/ best) |
| Re-encode / quality / bitrate controls | CloudConvert, Convertio, FreeConvert | ❌ intentionally none | **out of scope** — see below |
| Batch / multi-file | Convertio, FreeConvert | ❌ single input | out of model (page driver is single-upload) |
| Cloud-storage import (Drive/Dropbox) | CloudConvert, Convertio | ❌ | out of model |

### Why no re-encode / params (honest justification)

`mov-to-mp4` needed a `transcode` fallback because MP4 **cannot legally hold**
some MOV codecs (e.g. Apple ProRes). That does not apply here: **MKV is a
superset container** that accepts essentially every codec MP4 can carry
(H.264, HEVC, AV1, VP9, MPEG-4, AAC, AC-3, …). So a stream-copy remux from
MP4 → MKV **always succeeds** — there is no "codec that won't fit" case to fall
back on. Adding a re-encode mode would only *lose* quality and add a slower path
with no correctness need, so this tool is deliberately param-free. Users who
want to actually re-encode (change codec/quality) already have
`video-transcode` and `video-compress`. This keeps mp4-to-mkv a single-purpose,
lossless, instant remux — the thing the "remux without re-encoding" competitors
all advertise as their headline.

The server competitors' extra knobs (resolution, bitrate, cloud import, batch)
are either re-encode features (covered by other gizza tools) or infrastructure
gizza intentionally doesn't have (no server, single local input). No in-model
copy/UX/visual gap remained to close beyond a clear page (worked example,
limits, ≥3 FAQs) explaining the lossless-remux value and the "add subtitles /
audio tracks later" motivation for MKV.

## Sources

- https://cloudconvert.com/mp4-to-mkv
- https://convertio.co/mp4-mkv/
- https://www.freeconvert.com/mp4-to-mkv
- https://flonnect.com/media-tools/remux-container
- https://cutfa.st/features/mp4-to-mkv
- https://cococonvert.com/blog/informational__what-is-mkv-container
- https://imagekit.io/blog/mkv-vs-mp4/
- https://cloudinary.com/guides/video-formats/mkv-format-what-is-mkv-how-it-works-and-how-it-compares-to-mp4

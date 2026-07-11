# Competitor analysis — mkv-to-mp4 (2026-07-11)

Function: convert a Matroska `.mkv` video into an `.mp4` container. Findings are paraphrased from the scan; no competitor copy, branding, or trademarks are reproduced.

## Competitors scanned

1. **VideoProc-style desktop converters / remux guides** — emphasize the important distinction between a fast remux (`-c copy`) and a slower transcode. Table-stakes: preserve quality when codecs are already compatible, explain that a remux changes only the container, and keep a fallback path for incompatible codecs.
2. **Cubix-style online MKV→MP4 converter** — exposes explicit choices between preserving streams and re-encoding for broad playback compatibility. Table-stakes: one-click default conversion, a compatibility-oriented encode mode, and output that plays in browsers/devices.
3. **Flonnect-style browser remux tools** — advertise local WebAssembly processing and broad container conversion without upload. Table-stakes: browser-local privacy, fast container-only mode, and a clean download result.

## Table-stakes → implementation decision

| Table-stake | In/out of model | Landing |
|---|---|---|
| Convert MKV to MP4 | in-model | ffmpeg page + CLI/chat block returns `video/mp4` |
| Lossless no-reencode remux when streams are MP4-compatible | in-model | default `mode=copy` uses `-c copy` |
| Fallback encode for VP8/VP9/AV1/FLAC/Vorbis/Opus-style incompatible streams | in-model | `mode=transcode` uses libx264/AAC |
| Quality control for lossy fallback | in-model | `quality` 1-100 maps to practical CRF range |
| Browser-local/no-upload page | in-model | ffmpeg page runtime with uploaded file input |
| Preserve subtitles/attachments | out-of-model for MP4 remux | documented as dropped because MP4 cannot carry common MKV subtitle/attachment tracks |
| Batch conversion / multi-file queue | out-of-model | page supports one uploaded file per run |
| Automatic codec probing before choosing copy/transcode | out-of-model for current descriptor | user chooses mode; copy-mode ffmpeg error tells user to transcode |

## Design shipped

- Default **Remux** mode is lossless and fast: `-map 0:v? -map 0:a? -c copy -movflags +faststart out.mp4`.
- **Transcode** mode is the compatibility fallback: H.264 video + AAC audio, with `quality` mapped to CRF 40..18 (higher quality → lower CRF).
- Subtitle/data/attachment streams are intentionally not mapped. MP4 cannot legally carry most MKV subtitle and attachment tracks; keeping them would make many otherwise-valid conversions fail.
- Page UX includes examples for remux, high-quality transcode, and smaller transcode; the Playwright spec verifies a real MKV fixture converts to a decodable MP4 and that deep-link parameters prefill correctly.

## Out-of-model / not built

- Batch conversion and drag/drop queues.
- Automatic codec analysis with a suggested mode.
- Subtitle extraction or burn-in; use subtitle-specific/video-caption tools before conversion if needed.
- Advanced codec selection beyond MP4-compatible H.264/AAC fallback.

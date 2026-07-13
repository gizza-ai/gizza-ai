# Competitor analysis — video-extract-audio-track (2026-07-13)

Function: demux (stream-copy) the audio track out of a video and save it in its
**original codec without re-encoding** (lossless, near-instant). Distinct from the
existing `extract-audio-from-video` (which *re-encodes* to MP3/WAV) and from
`mp4-to-m4a` (fixed MP4→M4A only). This tool is the general lossless demuxer:
any input container, codec preserved, container chosen to fit the codec.

Research via WebSearch (2026-07-13). All findings paraphrased — no competitor
copy/branding used.

## Competitors surveyed

| tool | type | relevant behavior (paraphrased) |
| ---- | ---- | ------------------------------- |
| Flonnect Extract Audio | browser (wasm) | Browser-local extraction; offers lossless containers and a modern Opus/WebM path; auto-drops the video track. |
| Tiny-Online.Tools Extract Audio | browser (Web Audio) | Local, no-upload extraction; download the audio track. |
| HighTool Audio Extractor | browser | Format menu incl. "100% quality" lossless options. |
| LosslessCut | desktop (open source) | Core value = cut/extract **without re-encoding**; pulls the untouched audio stream from the container in seconds. |
| MKVToolNix / mkvextract | desktop GUI | Demux/remux individual tracks; **pick which track** (audio/video/subtitle) to extract from multi-track files. |
| FFmpeg (baseline) | CLI | `ffmpeg -i in -vn -c:a copy out.<ext>` byte-for-byte copies the audio stream with a **codec-matching extension** (.aac/.opus/.m4a…). |

## Table-stakes (each tagged in-/out-of-model)

| capability | in-model? | decision |
| ---------- | --------- | -------- |
| Lossless stream-copy (`-c:a copy`), no quality change | **in-model** | Core behavior. |
| Drop the video stream (`-vn`) | **in-model** | Always applied. |
| Output container matching the codec (M4A for AAC/ALAC, OGG for Vorbis/Opus, universal MKA for anything) | **in-model** | `container` enum, default `mka` (never errors on any codec). |
| Works from any input container (MP4/MOV/MKV/WebM/AVI…) | **in-model** | `Input::Video`, ffmpeg auto-detects the demuxer. |
| Pick which audio track on multi-track files (e.g. language tracks) | **in-model** | `track` integer param → `-map 0:a:<n>`, default 0. |
| Browser-local, no upload | **in-model** | gizza runs ffmpeg in-browser on the page; nothing uploaded. |
| Re-encode to lossy MP3 / lossless WAV / FLAC | **out-of-model here (by design)** | That is a *re-encode*, not this tool's promise. Covered by the sibling `extract-audio-from-video` (MP3/WAV) and `audio-convert` (mp3/wav/ogg/flac/m4a). Linked in the page copy; not built here. |
| Batch / many-file queue | out-of-model | No server/queue in the browser-local model. |
| Extract subtitle/video tracks too | out-of-model (scope) | This tool is audio-only by definition (`-vn`). |

## UX patterns adopted

- Container `<select>` (enumv) with friendly labels — MKA (any codec), M4A (AAC),
  OGG (Vorbis/Opus).
- Numeric track field defaulting to 0 (first track).
- Worked example + limits stated on the page (codec/container compatibility,
  single-stream selection, why MKA is the safe default).

## Gaps vs our first cut

Our tool ships every in-model table-stake from the start (container selection,
track selection, universal-lossless default). Out-of-model re-encode targets are
listed and cross-linked to the existing gizza tools, not rebuilt.
</content>
</invoke>

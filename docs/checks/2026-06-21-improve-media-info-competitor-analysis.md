# media-info — competitor analysis (2026-06-21)

Tool: `gizza-ai/media-info` — inspect an audio or video file and report its
container, codecs, duration, overall bitrate, and per-track stream metadata
(sample rate, channels/layout, bit depth) from the bytes, with no re-encoding.

Surfaces shipped: **chat** (pure-Rust wafer block, validated) + **CLI**
(`gizza tool media-info url=…`). **No standalone page** — a media-file → JSON
report fits neither the pure-text page nor the ffmpeg file→media page shape (the
established "no-page file-input" pattern, same as `image-info` /
`detect-file-type`). Engine is the pure-Rust `symphonia` demuxers, so it runs on
**all** backends including the chat Service Worker (no ffmpeg, which can't run in
a SW).

## Competitors surveyed

1. **Probe.video** — paste a URL, server-side ffprobe returns full metadata
   (codecs, bitrate, resolution, frame rate). No upload/storage claimed.
2. **EditClips Media Info** — ffprobe-backed: video codec, resolution, frame
   rate, bitrate, audio format, color space.
3. **QuickEditVideo Info** — ffprobe compiled to **WebAssembly, fully in-browser**
   (no upload): format, video codec, resolution, fps, bitrate, audio codec,
   sample rate, channels, duration.
4. **Ayrshare Video Probe** — ffprobe-based URL inspector: duration, dimensions,
   format, bitrate, codecs.
5. **inoRain ffprobe Online** — URL → codec info, bitrates, frame rates.

## Capability diff (competitors → media-info)

| Capability | Competitors | media-info | Status |
|---|---|---|---|
| Container/format name | yes | yes | matched |
| Duration (seconds + human) | yes | yes (`duration_seconds` + `duration`) | matched |
| Overall bitrate | yes | yes (`overall_bitrate_kbps`, derived size/duration) | matched |
| Per-track listing | yes | yes (`tracks[]`, indexed, with `track_count`) | matched |
| Audio codec | yes | yes (AAC/MP3/FLAC/ALAC/Opus/Vorbis/PCM/ADPCM) | matched |
| Sample rate | yes | yes | matched |
| Channels + layout | partial | yes (count + mono/stereo/5.1/7.1) | matched / ahead |
| Bit depth | partial | yes (`bits_per_sample`) | matched |
| File size | yes | yes (`bytes`) | matched |
| Reads by URL, no upload | yes | yes (CLI/chat fetch the URL; bytes never stored) | matched |
| JSON output | ffprobe-style | yes (clean flat JSON for the LLM) | matched |

## Gaps vs. competitors (out of model — NOT built)

These all require a **full video decoder / ffprobe**, which is out of model here
(gizza is pure-Rust + ffmpeg, and ffmpeg can't run in the chat SW; `symphonia`
is a metadata demuxer that does not decode video):

- **Video codec name + resolution + frame rate.** `symphonia` exposes a
  container's video stream only as an undecodable track, so media-info reports
  it honestly as `codec: "unsupported/other"`, `kind: "other"` rather than
  inventing H.264/resolution it can't read. Closing this would need an ffprobe
  surface (CLI-only at best; not in-chat).
- **Color space / pixel format / HDR metadata** — video-decoder territory.
- **Embedded tags / cover art / chapters** — partially demuxable but not
  surfaced; deferred to keep the report focused on technical stream metadata.

## Closed in-model gaps (this build)

- Honest video-track labelling: undecodable container tracks report
  `"unsupported/other"` / `kind: "other"` instead of being mislabelled as a
  known audio codec (fixed after observing the initial MP4 output).
- Channel layout naming (mono/stereo/2.1/5.1/7.1) added beyond a bare count.
- Human-readable duration alongside the numeric seconds.

## Verification

- Unit tests (core): 5 passed (real WAV parse, channel layouts, duration
  formatting, container sniff, error paths).
- Drift-guard schema test: passed (chat schema matches authored).
- `wafer build`: OK — `symphonia` **instantiates** in wasm32-wasip1 (1.35 MiB
  block.wasm), so the chat surface is functional.
- CLI live: WAV → `WAVE (RIFF)`, 6.307 s, 1536 kbps, stereo 48 kHz 16-bit PCM;
  MP4 → `MP4 / M4A (ISO BMFF)`, 10.027 s, 629 kbps, AAC tracks + honest
  `unsupported/other` for the video stream.

No competitor copy, branding, or trademarks were reused.

# mp4-to-m4a — competitor analysis (2026-07-11)

Tool: `mp4-to-m4a` extracts the first audio track from an MP4/M4V video into an
M4A container by stream-copying (`-vn -map 0:a:0 -c:a copy`). The goal is a
lossless, no-reencode audio remux, not codec conversion.

No live web search was available in the final implementation pass for this tool,
so this analysis names no specific competitors. It is based on common ffmpeg and
online video-to-audio converter table stakes, plus nearby gizza tools
(`extract-audio-from-video`, `audio-convert`).

## Table-stakes and decisions

| Capability | Common expectation | gizza decision | Fit |
|---|---|---|---|
| Extract audio from MP4 | Convert/upload an MP4 and get audio-only output | Output `.m4a` / `audio/mp4` | in-model |
| No quality loss option | Some tools advertise "no re-encode" when source audio is AAC/M4A-compatible | Always `-c:a copy`; no quality knob | in-model |
| Drop video | Audio-only output | `-vn` | in-model |
| Track selection | Pro tools expose multiple audio tracks | First audio stream only (`0:a:0`) | documented limit |
| Convert to MP3/WAV/other formats | Common in general converters | Out of scope; covered by extract-audio-from-video/audio-convert | out-of-model for this single-purpose tool |
| Batch/cloud import | Server converters | Not supported; browser page is single-file local input | out-of-model |

## Why this is not a duplicate

- `extract-audio-from-video` outputs MP3/WAV and re-encodes. `mp4-to-m4a` keeps
the original compressed audio packets with `-c:a copy`.
- `audio-convert` takes an audio input and transcodes to formats including M4A.
`mp4-to-m4a` takes a video input and extracts/remuxes its first audio stream.
- The value proposition is specifically "lossless audio remux from MP4 to M4A."

## Implemented UX

- Parameter-free descriptor: source URL/ref only, no misleading bitrate controls.
- Page copy explains lossless remux, first-track behavior, and no-audio failure.
- Playwright covers exact argv and a real browser ffmpeg audio output from a
small audio+video fixture.

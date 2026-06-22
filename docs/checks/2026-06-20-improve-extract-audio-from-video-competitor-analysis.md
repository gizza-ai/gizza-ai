# extract-audio-from-video — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/extract-audio-from-video` — pull the audio track out of a
video as MP3 (lossy) or WAV (lossless). Page + CLI (ffmpeg runtime; chat
registers the schema but ffmpeg can't run in the chat Service Worker).

## What competitors do

- **Online "video to MP3 / extract audio" sites** (123apps audio extractor,
  online-audio-converter, flixier, clideo, media.io) — upload a video, get an
  MP3/WAV/AAC. Strengths: many formats. Weaknesses: the file is **uploaded to a
  server** (privacy + size caps + queue waits), most gate higher bitrate or
  longer files behind a paywall, and many watermark or rate-limit.
- **Desktop tools** (VLC "Convert/Save", Audacity import, raw `ffmpeg -vn`) —
  full control but require installing software and knowing the CLI incantation.
- **ffmpeg one-liner** — `ffmpeg -i in.mp4 -vn -c:a libmp3lame -b:a 192k out.mp3`
  is exactly what this tool builds; the value we add is packaging it as a
  zero-install browser/CLI tool with sane defaults.

## How this tool competes / improves

1. **Runs locally — nothing is uploaded.** The page does the extraction in the
   browser via ffmpeg-wasm; the CLI runs it headless. Private by construction,
   no size queue, works offline once loaded.
2. **Lossless option.** Offers WAV (16-bit PCM) alongside MP3, so the output can
   be a perfect decode for editing/archiving — many free online extractors only
   give you a re-compressed lossy file.
3. **Bitrate control, free.** MP3 bitrate is selectable 32–320 kbps (default
   192) with no paywall; competitors commonly lock ≥192 kbps behind premium.
4. **`-vn` drops the video stream** so only audio is encoded (faster, smaller)
   rather than muxing a silent video track.
5. **Chainable** — the CLI/chat output is a normal media envelope, so the
   extracted audio can be referenced by a later tool via `ref`.
6. **Honest defaults & validation** — empty format defaults to MP3@192; bitrate
   is range-checked (32–320) and ignored for lossless WAV.

## Framework enhancement shipped with this tool

The page output infra previously rendered only `image`/`video` outputs. This
tool is the first **audio-output** tool, so it adds `format = "audio"` support:
- `tools/generator/src/template.rs` — renders an `<audio controls>` element (+
  download link) for `format == "audio"`.
- `site/tool-ffmpeg.js` — extends the extension→MIME map with mp3/wav/ogg/flac/
  aac/m4a/opus so the produced file gets the right `data:audio/…` URL.
- `site/tool.js` — the media-element handling was already element-agnostic
  (`media.src = dataUrl`); only the misconfig message was updated.

This unlocks the whole **video→audio** family for future tools. (Audio-**input**
tools — audio-convert, audio-normalize, … — still need an `AssetKind::Audio` +
`accept="audio/*"` page input, which remains future work.)

## Tests

8 core unit tests (format parse/default, ext+mime, bitrate clamp, mp3/wav argv,
plan) + the block drift-guard schema test. CLI verified over the wire on a video
with an audio track: MP3 (codec `mp3`, 128 kbps confirmed via ffprobe) and WAV
(codec `pcm_s16le`). Playwright page test uploads a tiny video-with-audio
fixture and asserts the output media src is a `data:audio/…` URL. (A video with
no audio track correctly fails — there is nothing to encode.)

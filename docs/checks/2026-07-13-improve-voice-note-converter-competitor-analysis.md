# voice-note-converter — competitor analysis (2026-07-13)

Tool: convert chat voice messages (Opus/OGG from WhatsApp/Telegram/Signal) to
mp3 or wav — and back to a real Opus voice note. Runs entirely in the browser
(page) / via ffmpeg-runtime (chat + CLI).

## Why this is NOT a duplicate of `audio-convert`

`blocks/audio-convert` writes mp3/wav/**ogg-vorbis**/flac/m4a — it has **no Opus
encoder**. A messaging-app voice note is the *Opus codec in an Ogg container*
(`.opus`); vorbis-in-ogg is not interchangeable with it. audio-convert can
*decode* an incoming `.opus` (ffmpeg auto-detects any input), so the forward
"voice note → mp3/wav" direction overlaps, but the **reverse "mp3/wav → Opus
voice note"** and the voice-tuned Opus settings (mono downmix, `-application
voip`, low bitrate) are the distinct, in-model capability this tool adds.
Confirmed `ffmpeg -encoders` ships `libopus`, and a wav→opus(voip,mono,24k)→mp3
round-trip works locally.

## Competitors scanned (paraphrased — no copy/branding reused)

1. **CoolUtils OPUS→MP3** — server-side, files up to 50 MB, single direction
   (opus→mp3 only), no bitrate control surfaced.
2. **CleverUtils WhatsApp/Telegram voice→MP3** — server-side, up to 100 MB,
   HTTPS + auto-delete; publishes an "Opus bitrate guide" (voice 32–64 kbps
   clean, music 96–128 kbps).
3. **XConvert OPUS→MP3** — accepts `.opus` **or** `.ogg`, bitrate options
   192/256 kbps, batch conversion (server-side).
4. **ImageOnline / EchoWave** — client-side, in-browser, no upload, no signup,
   no watermark; opus↔mp3 both directions.
5. **Convertio MP3→OPUS** — server-side both directions, exposes an Opus
   bitrate selector.

Reference specs surfaced: WhatsApp records voice notes as Opus **16 kHz mono
≈16 kbps**; Opus supports 6–510 kbps; "voice clean" ≈ 32–64 kbps, "music" ≈
96–128 kbps.

## Table-stakes → decisions

| Capability | Competitors | Our decision (in/out of model) |
|---|---|---|
| Decode voice note (opus/ogg) → mp3 | all | **in-model** — `format=mp3` |
| Decode voice note → wav (editable) | most | **in-model** — `format=wav` |
| Encode mp3/wav → Opus voice note | Convertio, EchoWave | **in-model** — `format=opus`, libopus |
| Bitrate control | XConvert, Convertio | **in-model** — `bitrate` param (per-format clamp + default) |
| Mono downmix (voice-note standard) | implied | **in-model** — `mono` boolean (default on) → `-ac 1` |
| Voice-tuned Opus (`-application voip`) | (specialist) | **in-model** — auto when mono, `audio` when stereo |
| Accept `.opus` OR `.ogg` input | XConvert | **in-model** — ffmpeg auto-detects any decodable input |
| Voice / music presets | CleverUtils guide | **in-model** — `[[example]]` preset chips |
| Batch (many files at once) | XConvert, CoolUtils | **out-of-model** — page/chat take one file per run |
| Files > 10 MiB | 50–100 MB | **out-of-model** — 10 MiB input cap (voice notes are tiny) |
| Trim / effects / transcription | — | **out-of-model** — other gizza tools / needs a model |

## Copy / UX patterns adopted (paraphrased, not copied)

- Present both directions as one tool ("to mp3/wav and back").
- Preset chips: **WhatsApp voice note** (opus, mono, 24k), **Music-quality
  Opus** (opus, stereo, 96k), **To MP3** (mp3, 128k), **To WAV** (wav).
- State the privacy/local-processing story (page runs ffmpeg-wasm locally).
- Explain that decoding to mp3/wav can't restore fidelity Opus already dropped.

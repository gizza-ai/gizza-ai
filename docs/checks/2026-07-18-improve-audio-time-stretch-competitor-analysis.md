# audio-time-stretch — competitor analysis (2026-07-18)

Function: speed audio up or down (change tempo/BPM) while keeping the pitch unchanged —
"time stretch". The inverse of audio-pitch-shift (which changes pitch, keeps tempo).

## Competitors skimmed (top 3 + others)

1. **Music Speed Changer** (app.musicspeedchanger.com) — real-time speed + pitch on phone,
   tablet, desktop. Time-stretch (change speed, pitch preserved) is one mode; pitch-shift is
   the other. Works online/offline; playback-focused with export.
2. **SoundTools Tempo Changer** (soundtools.io/tempo-changer) — free speed/BPM changer, slow
   down or speed up without changing pitch. Downloads in the SAME format uploaded — MP3, WAV,
   FLAC, AAC, OGG. Percentage/factor control.
3. **OnlineToneGenerator Time Stretcher** — change tempo of mp3/wav without affecting pitch;
   simple factor/percentage input.
4. KeyPitch — stretch ×0.5 to ×2 without pitch change; MP3/WAV/M4A/MP4; preview + download.
5. Tembrica — time-stretch to a target BPM (auto-detect source BPM), preview, download.
6. 29a.ch TimeStretch Player — loop/slow/speed sections for practice/transcription (player, no
   export).

## Table-stakes params / defaults / UX (paraphrased)

- **Speed/tempo factor** is the primary control. Common accepted range ×0.5–×2 (KeyPitch),
  some wider. `1.0` = unchanged. Presets: 0.5×, 0.75×, 1.25×, 1.5×, 2× are ubiquitous
  (slow-down for transcription/practice, speed-up for podcasts/audiobooks). → in-model
  (ffmpeg `atempo` chain, exactly what change-speed's audio path does; pitch is inherently
  preserved by atempo). We support **0.25–4** (wider than most, chained atempo).
- **Percentage vs factor** — some phrase it as % (150% = 1.5×). Same knob; we take the factor
  and document the % mapping in copy. → in-model (copy only).
- **Output format** — MP3/WAV/FLAC/AAC(M4A)/OGG, several default to "same as input". → in-model
  (enum mp3|wav|ogg|flac|m4a, default mp3; matches the audio family).
- **Preset chips** for common speeds. → in-model (`[[example]]` chips).
- **Local/offline processing, no upload** — table stakes for the privacy-first framing. → in-model
  (ffmpeg-wasm on the page; nothing uploaded).
- **Real-time preview / A-B looping** (Music Speed Changer, 29a.ch) — interactive scrub/loop
  player. → OUT of model here (our page is one-shot process→download, not a live player).
- **Automatic BPM detection → target BPM** (Tembrica) — needs a beat-tracking model/analysis.
  → OUT of model (no BPM detector). Manual: a user who knows source & target BPM sets
  factor = target/source; documented in copy.

## Decisions

- Params: `factor` (number, 0.25–4, required, presets) + `format` (enum, default mp3).
- atempo-only chain on an audio input (`-vn` to drop album-art video stream), re-encode to the
  chosen format. Pitch is preserved by construction (atempo is a WSOLA time-stretch).
- Presets: 0.5×, 0.75×, 1.5×, 2× (+ a 1.25× podcast preset).
- Copy documents %↔factor and BPM (factor = target BPM / source BPM); lists live-preview and
  auto-BPM as out of scope.

Sources: OnlineToneGenerator, app.musicspeedchanger.com, soundtools.io/tempo-changer,
elysiatools.com, keypitch.app, tembrica.com, 29a.ch/timestretch (paraphrased; no copy reused).

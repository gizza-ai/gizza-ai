# audio-noise-reduce — competitor analysis (2026-07-11)

Scan of the top online "remove background noise from audio" tools, done **before**
implementation. All notes are paraphrased — no competitor copy/branding was reused.

## Competitors reviewed

1. **Adobe Podcast — Enhance Speech (v2)** — browser AI speech cleaner that strips
   noise/reverb and re-synthesizes voice to a "studio" sound. One **Enhancement
   Strength** slider (0–100%, sweet spot ~30–50%); the slider is subscription-gated,
   free tier is a fixed full-strength pass. In: WAV/MP3/AAC/MP4 (+size cap). Out:
   WAV/MP3. No algorithm choice, no EQ/hum toggles — pure ML. Before/after playback.

2. **Media.io — AI Noise Reducer** — closest direct file→cleaned-file competitor.
   Adjustable **reduction strength** + custom decibel limit, a **noise-type picker**
   (wind/reverb/hiss) or one-click auto denoise, instant preview. In: MP3, WAV, M4A,
   OGG, AU + video (MP4/MOV/MKV…), ~2 GB cap. Out: MP3/M4A/OGG/WAV/FLAC (compressed
   vs lossless, MP3 up to 320 kbps). Cloud-processed, files deleted after 24h.

3. **Audioalter — Noise Reducer** — minimalist free tool, **no settings at all**, a
   single auto pass tuned for voice. In: MP3/WAV/FLAC/OGG, 50 MB cap. Upload → process
   → download. Simplicity is the pitch.

   *(Also skimmed: VEED one-click "Clean Audio" toggle (Dolby, ML/cloud/account);
   Cleanvoice podcast editor with a `remove_noise` boolean, `studio_sound` mode,
   preserve-music flag, loudness normalization, saveable presets — ML/cloud/account.)*

## Table-stakes params + typical defaults

| Param | Typical default | In our tool? |
|---|---|---|
| Noise-reduction **strength** (slider) | ~40–50% / "full" | ✅ `strength` 1–100, default 12, slider control |
| One-click / **auto** default | on | ✅ ships a sensible default strength |
| **Algorithm / mode** choice | auto | ✅ `method` afftdn / anlmdn (a differentiator) |
| **High-pass / hum removal** | off / folded in | ✅ `remove_hum` → 80 Hz high-pass |
| **Output format** choice | MP3 | ✅ mp3/wav/ogg/flac/m4a |
| Before/after **preview** | on | ~ page shows the result in an `<audio>` player |
| Room-tone preservation | ~10–15% (implicit in strength) | ✅ implicit — lower strength keeps more |
| Loudness normalization | off | out-of-scope here (see audio-normalize block) |

## Decisions (in-model vs out-of-model)

**Built (in-model, all table-stakes):**
- `strength` 1–100 slider → mapped to `afftdn nr` (×0.97 dB) or `anlmdn s` (÷1000).
- `method` enum afftdn / anlmdn — exposing the algorithm out-differentiates the
  single-pass competitors.
- `remove_hum` boolean → `highpass=f=80` prepended — a hum/rumble win most rivals
  fold silently into their model.
- `format` enum mp3/wav/ogg/flac/m4a (matches Media.io's breadth; beats MP3-only tools).
- Fully local wasm processing (no upload) — structural privacy edge over the cloud tools.

**Considered, NOT built (out-of-model — needs ML / cloud / account):**
- AI speech re-synthesis / "studio sound" (Adobe Enhance, Cleanvoice `studio_sound`,
  VEED/Dolby) — deep-learning voice reconstruction.
- ffmpeg `arnndn` RNN denoise — ffmpeg-native but requires an external trained
  `.rnnn` model file we don't bundle; out-of-model unless a model ships.
- Noise-type auto-detection (wind vs reverb vs hiss models) — the classification is
  ML. Note: the *hum/rumble* sub-case is partially covered by our high-pass toggle.
- Dereverb / echo removal — no robust ffmpeg-native dereverb.
- Filler-word / breath / click removal, transcript editing (Cleanvoice) — ASR/ML,
  scope creep beyond denoising.
- Cloud accounts, login-tied saved presets, multi-GB uploads — irrelevant to a
  local browser tool. (Static one-click example chips are provided instead.)

## Relationship to existing blocks

`blocks/video-audio-denoise` denoises the audio track of a **video** (Input::Video,
keeps the picture with `-c:v copy`; rejects audio-only files). This tool is the
**audio-input** counterpart (Input::Audio) for the audio-* family — a user with a
`.mp3`/`.wav` cannot use the video block. Same denoise engine philosophy, different
media class and page (`accept="audio/*"`, `format="audio"`), plus an output-format
selector the video tool doesn't need (it keeps the input container). Not a duplicate.

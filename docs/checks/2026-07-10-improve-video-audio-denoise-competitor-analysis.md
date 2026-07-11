# video-audio-denoise — competitor analysis (2026-07-10)

Tool function: reduce background hiss/hum/noise in a **video's audio track** with
ffmpeg's `afftdn` (FFT) / `anlmdn` (non-local means) denoiser while stream-copying
the picture (`-c:v copy`). Surfaces: standalone page + CLI (chat ffmpeg is
unavailable in the Service Worker). Sibling of the built `video-audio-gain`.

## Competitors scanned

1. **SoundTools — Noise Remover** (soundtools.io/noise-remover) — single **strength
   slider**; "reduce the strength until the audio sounds natural, most recordings
   sound best at 50–75%"; A/B preview; 100% in-browser, files never uploaded.
2. **Descript — Remove Background Noise / Studio Sound** — one **intensity slider**
   ("dial it up or down"), keeps voice natural. ML-based studio-sound model.
3. **MyEdit — Remove Background Noise** — adjustable **Noise Reduction** + a
   **Compensation** (make-up gain) control, HQ export. ML-based.
4. **Media.io Noise Reducer** / **MP3Cut Denoise** / **VEED** — one-click "remove
   noise" on audio *or* video; VEED/MP3Cut explicitly process a video's audio track
   and return the video; mostly a single strength/one-click control.

## Table-stakes params, defaults, worked examples

| Capability | Competitors | Our decision | In/out of model |
|---|---|---|---|
| Intensity/strength control | slider, ~40–75% sweet spot | `strength` 1–100 slider, default 12 (conservative) → maps to `afftdn nr` dB / `anlmdn s` | **in** |
| Keeps the video, audio-only change | VEED/MP3Cut | `-c:v copy` always; only audio re-encoded | **in** |
| Denoiser method / quality choice | Descript studio vs. classic | `method` = `afftdn` (FFT, default) or `anlmdn` (non-local means) | **in** |
| Remove low hum/rumble | implicit in "remove hum" | `remove_hum` toggle → `highpass=f=80` prepended | **in** |
| Fully in-browser / private | SoundTools, most | ffmpeg runs in the page tab; nothing uploaded | **in** |
| A/B before/after preview | SoundTools, Descript | out — page shows the processed result + download only | **out-of-model** (UI feature, not a param) |
| Make-up gain / "Compensation" | MyEdit | out — use the sibling `video-audio-gain` tool to boost level after | **out-of-model** (compose with existing tool) |
| ML "studio sound" voice model | Descript, MyEdit | out — gizza is pure-Rust + ffmpeg, no ML model | **out-of-model** (documented, not built) |
| Noise-profile / "learn noise sample" | some pro tools | out — afftdn adapts its own floor (`nr`/`nf` defaults); no sample-region UI | **out-of-model** |

Every table-stake is either in the descriptor (`method`, `strength`, `remove_hum`,
implicit video-copy) or listed above as out-of-model. Nothing dropped silently.

## Design notes

- **strength → filter mapping** (deterministic, unit-tested): afftdn `nr = strength ×
  0.97` dB (strength 100 → 97, the afftdn max; 12 → 11.64); anlmdn `s = strength /
  1000` (12 → 0.012, 100 → 0.1) — anlmdn's native default 0.00001 barely denoises, so
  the slider maps into its useful 0.001–0.1 band.
- Conservative default (strength 12) matches the competitor advice to start low and
  raise gradually; over-processing sounds robotic.
- `remove_hum` default **off** (a highpass can thin wanted bass) — the page tests the
  non-default (on) path.
- No copy/branding/trademarks reproduced from any competitor; wording is original.

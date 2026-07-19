# audio-pause-shortener — competitor analysis (2026-07-17)

Function: tighten speech pacing by detecting long quiet gaps and shortening each gap to a natural maximum instead of deleting all silence.

## Competitors scanned (paraphrased)

1. **Audacity Truncate Silence** — exposes a silence threshold, a minimum duration that counts as silence, and a target retained duration. It shortens long gaps while preserving a little room tone.
2. **Descript / podcast editors** — market this as shortening word gaps or removing filler dead air; typical UX uses a sensitivity/threshold and a maximum pause length, with presets for natural vs tight pacing.
3. **VEED / online silence remover tools** — upload audio/video, choose silence threshold and removal/shortening intensity, then export in common audio/video formats.

## Table-stakes params

| Capability | Competitor norm | Fit | Decision |
|---|---|---|---|
| Silence threshold | threshold/sensitivity around -30 to -40 dB | in-model | `threshold_db`, default -30, maximum 0 |
| Trigger length | minimum silent gap before editing | in-model | `max_pause`, default 1.5 s |
| Retained gap length | keep a natural pause rather than hard cut | in-model | `target_pause`, default 0.5 s and must be less than `max_pause` |
| Output format | mp3/wav/ogg/flac/m4a common | in-model | `format` enum |
| Preserve short pauses | expected for natural speech | in-model | STOP-side `silenceremove`; short gaps pass through |
| Live transcript-aware word-gap editing | Descript-style editor | out-of-model | no speech recognition/transcript model |
| Visual timeline preview | editor tools | out-of-model | page runs ffmpeg and returns audio, no waveform editor |

## Design notes

This differs from `audio-silence-remove`: that sibling strips silence down to a fixed beat, including leading silence. This tool caps only pauses longer than `max_pause`, leaving short pauses and leading silence intact for natural pacing.

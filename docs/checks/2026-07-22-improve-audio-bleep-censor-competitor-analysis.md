# audio-bleep-censor — competitor analysis (2026-07-22)

Function: take an audio file and censor one or more time regions — replace each
region with a bleep tone, silence, or a lowered "duck" level — to hide swear
words or sensitive content.

## Competitors skimmed (paraphrased notes only — no copy/branding reused)

1. **Bleep That Sh\*t!** (bleepthat.sh/censor-audio) — browser tool. Upload
   MP3/MP4, auto-transcribes with Whisper to get word-level timestamps, then you
   click words to censor. Offers a small library of censor sounds: classic TV
   bleep, brown noise, novelty sounds (dolphin/T-Rex), or silence. Runs locally.
2. **Choppity** (choppity.com profanity-censoring) — auto profanity detection on
   the transcript, then a choice of three censor styles: classic **bleep**,
   silent **mute**, or a softer **volume-adjusted** (duck) censor. Batch/paid.
3. **VSilencer** (iOS) — "Time Range Mode": pick explicit start/end ranges and
   replace each with silence or a bleep sound (also has an auto profanity mode).
   The manual time-range flow is the directly comparable, model-fittable one.

(Bleep-Be-Gone / Adobe Premiere text-based bleep were also seen; Premiere = the
same mute-or-bleep-a-region idea driven by its transcript editor.)

## Table-stakes params → decision

| Capability | Decision |
|---|---|
| Censor explicit **time regions** (multiple) | IN — `regions`, comma list of `start-end`, seconds or `mm:ss`/`hh:mm:ss` |
| **Bleep** with a tone | IN — `mode=bleep`, classic ~1 kHz sine mixed over the muted region |
| **Silence / mute** a region | IN — `mode=mute` (volume 0) |
| **Softer / volume-adjusted** censor | IN — `mode=duck` (drops the region to ~-20 dB, keeps a trace) |
| **Bleep tone frequency** | IN — `tone_hz` (100–8000, default 1000 = classic TV bleep) |
| Output format choice | IN — `format` mp3/wav/ogg/flac/m4a (family standard) |
| Multiple regions at once | IN — up to 50 regions per run |
| **Auto profanity / speech-to-text** detection | OUT-of-model — needs Whisper/ML; gizza is pure Rust + ffmpeg. Listed, not built; the manual time-region flow is the shipped scope. |
| **Sound-effect library** (brown noise, novelty sounds) | OUT-of-model — needs bundled audio samples; we ship a tunable sine bleep instead. |
| Video input (bleep a video's audio) | OUT here — separate video tool; this one is audio-in/audio-out. |

## UX controls competitors ship (page mirrors the fittable ones)

- Preset censor-style buttons (bleep / mute / softer) → `mode` enum select +
  `[[example]]` preset chips on the page.
- Frequency is usually fixed at the classic 1 kHz; we expose it as a field
  with the 1000 default so it stays one-click but is tunable.
- Region picking in competitors is transcript-click (out of model); ours is an
  explicit `start-end` list, documented with worked examples.

## Worked defaults chosen

- `mode` default **bleep** (the expected censor sound), `tone_hz` **1000**.
- `regions` required, e.g. `1.5-2.0, 0:07-0:08.5`.
- Bleep implementation: mute the voice inside each region and mix a gated sine
  over just those regions (`amix`), so speech outside the regions is untouched.
- `mute` = `volume=0` on the regions; `duck` = `volume≈0.1` on the regions.

# audio-volume-adjust — competitor analysis (2026-07-02)

One WebSearch ("increase audio volume online mp3 louder tool dB gain"); skimmed the top real
tools: MP3Louder, onlineconverter increase-mp3-volume, Notevibes volume booster, mp3cut.net
change-volume, audiotrimmer volume-booster, SafeAudioKit, SoundTools, DuneTools, audioalter.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Gain in dB (±20 typical, default ~+3/+6) | MP3Louder (default 3 dB), SafeAudioKit (±20), DuneTools (±12 presets) | in-model | `amount` number, `unit=db` default; range widened to ±60 for edge uses; page placeholder +6 |
| Gain by percentage/factor (200% = double) | onlineconverter, DuneTools | in-model | `unit=factor`, amount in (0, 16] |
| Clipping protection while boosting ("limit peaks to 0 dB") | mp4gain, DuneTools | in-model | `limiter` boolean, default ON → `alimiter` stage after `volume` |
| Local/in-browser processing | Notevibes | in-model | how gizza pages work; stated on page |
| Sliders/preset buttons (+6/+12) | DuneTools, mp3cut | out-of-model | page framework renders plain fields; placeholder + copy carry the presets |
| Output format choice | most | in-model | family-standard `format` enum, default mp3 |

## Design decisions

- No-op values are rejected with a guiding message (0 dB / factor 1) instead of silently
  re-encoding: a user who typed them wanted a change.
- Page empty amount (arrives as 0.0) maps to the +6 dB placeholder for the db unit only —
  for factor, 0 is a real validation error and surfaces as one.
- Cross-reference audio-normalize in copy/FAQ: fixed gain vs measured loudness target is the
  #1 user confusion competitors' blogs answer; both tools link the distinction.
- Verification measures real gain: CLI compares ffmpeg volumedetect mean_volume before/after a
  -10 dB cut (measured exactly -39.0 → -49.0 dB); the page tests decode BOTH the fixture and
  the output with WebAudio and assert the RMS ratio (×2 for +6 dB, ×0.5 for factor 0.5).
  Ratio-based assertions were forced by a real gotcha: lavfi sine fixtures sit ~18 dB below
  full scale, so absolute RMS windows fail (now recorded in references/page-patterns.md).
- alimiter options are pinned explicitly (`limit=1:level=disabled`) so behavior can't drift
  with build defaults.

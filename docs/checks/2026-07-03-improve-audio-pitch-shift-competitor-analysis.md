# audio-pitch-shift — competitor analysis (2026-07-03)

One WebSearch ("online pitch shifter change audio pitch semitones without changing tempo
tool"); skimmed the top real tools: Audioalter pitch-shifter, SoundTools pitch-shifter,
vocalremover.org pitch, Tembrica pitch, AudioSpeedChanger, x-minus.pro transpose.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Shift by semitones, up or down | all of them | in-model | required `semitones` number param |
| Tempo/duration preserved | all (the tool's definition); Tembrica exposes it as a "lock tempo" toggle | in-model | always on — asetrate+aresample+atempo chain; change-speed is the sibling for the speed case |
| ±24 semitone range | Audioalter (two octaves); most others ±12 | in-model | ±24, atempo chained per instance beyond ±12 |
| Fine/fractional shifts (cents) | vocalremover.org (fine slider) | in-model | `semitones` is f64 — 0.5 = 50 cents, 0.01 = 1 cent |
| Common output formats | SoundTools (mp3/flac/wav/aac/ogg) | in-model | family-standard `format` enum mp3/wav/ogg/flac/m4a, default mp3 |
| Real-time preview while dragging | vocalremover.org, Tembrica | out-of-model | page framework is run-per-change, no live audio graph |
| Key detection / suggest target key | vocalremover.org | out-of-model | needs pitch detection (ML-ish); FAQ teaches counting semitones instead |
| Formant-preserving voice mode | (pro/studio shifters) | out-of-model | no rubberband in either ffmpeg build; FAQ states the ±4-semitone natural range honestly |
| Combined speed+pitch control | AudioSpeedChanger, x-minus | out-of-model here | deliberate split: change-speed owns tempo; page copy cross-links the distinction |

## Design decisions

- Stock-ffmpeg chain (`aresample=44100,asetrate=<rate>,aresample=44100,atempo=<44100/rate>`)
  — librubberband is absent from both the native runtime and @ffmpeg/core, so the resample
  trick is the only in-model implementation. atempo is computed from the ROUNDED asetrate so
  duration is exact; residual pitch error < 0.04 cents.
- atempo chained (0.5..2 per instance) so the full ±24 range works on conservative builds;
  a unit test pins the +24/-24 chains.
- semitones=0 (the empty page field) rejected with a guiding error that names both
  directions and points format-only users at audio-convert.
- Family invariants kept: shared Format enum + 192k lossy codec args, `-vn`, 10 MiB caps,
  `-pitch-shifted.<ext>` filename suffix, drift-guard schema test.

## Verification (all run, all green)

- Unit: 12 core + 2 block tests (argv exactness, chain bounds, rounding, errors, drift guard).
- CLI vs the public 1.26 s beep: +12 → window zero-crossing freq 2110→4075 Hz (×1.93 on a
  harmonic-rich beep), duration 1.254→1.201 s (preserved, not halved); -12 → 1057 Hz vs
  ~1055 Hz expected, duration 1.260 s exact. Exact-text no-op and range errors exercised.
- Playwright on the 440 Hz sine fixture: +12 → measured ~880 Hz still ~3 s; deep link
  `?semitones=-12&format=wav` → ~220 Hz still ~3 s; bare upload → guiding no-op error.
  Bounds pre-measured with local ffmpeg (880.0 / 220.0 Hz on the same chain).

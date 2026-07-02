# audio-eq — competitor analysis (2026-07-03)

One WebSearch ("audio equalizer online tool bass treble boost adjust"); skimmed the top real
tools: mp3cut.net equalizer, audioeditor.org 3-band EQ, mp3cut.org equalizer, ToolsCrow audio
equalizer, imagetoolhub 10-band EQ, AudioAlter (EQ + separate bass booster).

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| 3-band bass/mid/treble EQ, boost or cut | audioeditor.org (exactly this), ToolsCrow | in-model | `bass`/`mid`/`treble` dB params, ±20 |
| dB gains, positive = boost / negative = cut | audioeditor.org | in-model | same convention, 0 = band untouched |
| Bass-boost as a marquee use-case | AudioAlter (separate tool) | in-model | covered by `bass=6`; page copy names it |
| Local/in-browser processing | audioeditor.org ("files stay on your device") | in-model | how gizza pages work; stated on page |
| Output format choice (mp3/m4a/flac/wav) | mp3cut.net | in-model | family-standard `format` enum, default mp3 |
| Genre presets (rock, jazz, …) | mp3cut.net | out-of-model for v1 | fixed three-band params; copy suggests starting values instead |
| 10/31-band graphic EQ | imagetoolhub | out-of-model | three fixed bands is this tool's scope; a multi-band tool would be its own backlog entry |
| Real-time preview while adjusting | ToolsCrow | out-of-model | page framework is run-per-change, no live audio graph |

## Design decisions

- Fixed ffmpeg band shapes so the three sliders behave like classic tone controls:
  `bass=g=N` (low shelf, ~100 Hz corner), `equalizer=f=1000:t=q:w=1:g=N` (1 kHz peak),
  `treble=g=N` (high shelf, ~3 kHz corner). Zero-gain stages are omitted from the chain.
- All-zero requests are rejected with a guiding error (like audio-volume-adjust's 0-dB rule):
  a no-op lossy re-encode only degrades the file.
- Gains capped at ±20 dB and out-of-range values error, naming the offending band.
- Verification is spectral and pre-measured with local ffmpeg: a -15 dB bass cut on a 50 Hz
  tone scales RMS ×0.204 (-13.8 dB measured — the tone sits near the shelf corner), a +12 dB
  treble boost on an 8 kHz tone ×3.9 (+11.8 dB); Playwright asserts those ratios via WebAudio
  decode. CLI check: treble=-20 on a public beep moves volumedetect mean from -39.0 to
  -45.6 dB, matching the local ffmpeg measurement exactly.

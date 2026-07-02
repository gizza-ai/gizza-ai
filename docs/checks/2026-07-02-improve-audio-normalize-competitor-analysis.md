# audio-normalize — competitor analysis (2026-07-02)

One WebSearch ("normalize audio loudness online LUFS tool spotify podcast level"); skimmed the
top real tools: LoudFix, editingtools.io loudness normalizer, ImageToolHub audio normalizer,
Podtools loudness analyzer, EditText audio normalizer, Loopaloo volume normalizer.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Platform LUFS targets (-14 streaming, -16 podcast/Apple, -23 EBU, -24 ATSC) | editingtools, LoudFix, ImageToolHub | in-model | `lufs` number param, default -14, range -70..-5 (loudnorm's limits); targets named in the description + page copy |
| Local/WASM processing ("audio never leaves your device") | LoudFix | in-model | already how gizza pages work; stated on page |
| Peak / RMS normalization modes | ImageToolHub, EditText | out-of-model for v1 | LUFS (loudnorm) only; peak/RMS listed as later additions |
| Loudness ANALYSIS meters (LUFS readout before/after) | Podtools, editingtools | out-of-model | page framework has no meter UI; the chat surface reports the target applied |
| Output format choice | most | in-model | family-standard `format` enum mp3\|wav\|ogg\|flac\|m4a, default mp3 |

## Design decisions

- Single-pass `loudnorm=I=<lufs>:TP=-1.5:LRA=11`. Two-pass (measure, then normalize with
  measured values) lands closer on very dynamic material but needs an ffmpeg measurement run
  first — out of reach for the pure argv-plan model; the page copy states the tradeoff.
- TP fixed at -1.5 dBTP and LRA at 11 LU (the values streaming/podcast guides recommend);
  the one knob is the target LUFS. Keeps the form simple.
- Page empty-lufs field arrives as 0.0 (outside loudnorm's range) — treated as "use default
  -14" instead of erroring, so the common leave-it-blank flow works.
- CLI verification measures the OUTPUT loudness with local ffmpeg loudnorm print_format and
  asserts it lands near the requested target.

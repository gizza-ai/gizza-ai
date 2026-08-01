# audio-resampler — competitor analysis (2026-07-27)

New `/create-next-tool` backlog pick. ffmpeg audio tool — surfaces **chat + CLI +
page** (the generated ffmpeg-media page). One WebSearch ("change audio sample
rate online 44100 48000 16000 resample tool"); skimmed the top real tools:
online-convert.com "change sample rate", AudioAlter sample-rate changer,
mp3smaller/mconverter rate options, restream/veed audio sample-rate helpers,
and desktop Audacity's resample dialog for the capability baseline.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Pick a target sample rate (Hz) | online-convert, AudioAlter, all | in-model | `rate` integer, 3000–384000 Hz |
| Preset menu of common rates (8k…192k) | online-convert dropdown, Audacity | in-model | copy + `COMMON_RATES` name 8000…192000; presets suggested, any integer allowed |
| High-quality / anti-aliased resampling | Audacity ("best quality" sinc), pro tools | in-model | ffmpeg `swresample` (`-ar`) is windowed-sinc with anti-alias low-pass by default |
| Output format choice | online-convert, AudioAlter | in-model | family-standard `format` enum wav/flac/mp3/ogg/m4a, default lossless wav |
| Speech rates (8k/16k) called out for transcription | restream, veed voice helpers | in-model | copy names 8k/16k for speech; both are valid presets |
| Local / in-browser processing | (gizza differentiator) | in-model | page runs ffmpeg-wasm locally; stated in copy |
| Bit-depth control (16/24/32-bit) | online-convert, Audacity | out-of-model for v1 | wav writes 16-bit PCM; a bit-depth param is a sized follow-up, kept out to keep the schema focused on rate |
| Per-format bitrate control | online-convert | out-of-model | that's audio-convert's job; lossy targets here use a fixed transparent 192 kbps |
| Channel / mono-downmix in the same dialog | some converters bundle it | out-of-model | audio-to-mono / audio-channel own that; one tool, one job |
| Batch / multi-file | online-convert (paid) | out-of-model | single-input page + descriptor model, one file per call |

## Design decisions

- **Rate is the point, format is storage.** Distinct from audio-convert (changes
  container/codec at a fixed rate): here `-ar <rate>` drives ffmpeg's swresample,
  and `format` only decides how the resampled audio is stored. Default is
  lossless WAV so the resample isn't degraded by a lossy re-encode.
- **`-vn` always.** Album-art rides as an attached-picture video stream that
  audio-only muxers (wav especially) choke on; dropping it keeps every format
  working. Covered by a unit test across all five formats.
- **Wide but bounded range.** 3000–384000 Hz spans telephony (8k) through studio
  (192k) with headroom; out-of-range values error with a guiding message naming
  the accepted window and examples (44100 / 48000).
- **Argv ordering guarded.** `-ar` must precede `-c:a` so the encoder sees the
  resampled rate; a unit test asserts that ordering for every format.
- **Verification is header-exact.** Playwright resamples a 3 s tone to 16000 Hz
  WAV and parses the WAV header (offset 24) to assert the output rate is exactly
  16000, plus a WebAudio decode to confirm the full ~3 s duration survives. The
  deep-link test prefills `rate=48000&format=flac`, produces a `data:audio/flac`
  output, and confirms duration is preserved. CLI check resamples a real public
  audio file and confirms a valid resampled file plus the MIME/error guards.

> Original work only — no competitor copy, branding, or trademarks copied.

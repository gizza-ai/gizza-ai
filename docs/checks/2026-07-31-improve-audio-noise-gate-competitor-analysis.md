# audio-noise-gate — competitor analysis (2026-07-31)

Tool: gate audio below a threshold to silence or attenuate background noise during quiet passages.
Type: ffmpeg/audio. Notes are paraphrased; no competitor copy or branding is reused.

## Competitor scan

### 1. Audacity noise gate effect
- **Function:** applies a gate to reduce sound below a configured threshold.
- **Features:** threshold, level reduction, attack/decay timing, hold-like behavior, preview workflow.
- **Input/output:** local audio track to edited audio.
- **UX:** numeric controls in dB/ms and preview/apply flow.

### 2. FFmpeg `agate` filter documentation/examples
- **Function:** dynamic audio gate with level thresholding.
- **Features:** threshold, range/floor, ratio, attack, release, detection mode, upward/downward mode, link/channels options.
- **Input/output:** media file through filter graph to encoded audio.
- **UX:** command-line parameters; requires understanding linear threshold/range values.

### 3. Online audio cleanup / vocal gate tools
- **Function:** browser or cloud tools marketed for cleaning pauses and room noise.
- **Features:** upload audio, choose intensity/threshold, preview/download, commonly output MP3/WAV.
- **UX:** simple upload field, slider-like strength controls, format choice, examples focused on voice and podcasts.

## Table-stakes distilled

| Capability | In/out of model | Decision |
| --- | --- | --- |
| Audio upload input | in-model | built (`Input::Audio` / page file input) |
| Threshold control in dB | in-model | built (`threshold`, -80..0 dB) |
| Closed-gate reduction/floor | in-model | built (`reduction`, 0..80 dB; 0 rejected as no-op) |
| Ratio / slope control | in-model | built (`ratio`, 1..20) |
| Attack and release timing | in-model | built (ms controls) |
| RMS vs peak detector | in-model | built (`detection=rms|peak`) |
| Output format selection | in-model | built (`mp3|wav|ogg|flac|m4a`) |
| Browser-local processing | in-model | built through page ffmpeg runtime |
| Spectral denoising/noise-profile learning | out-of-model for this tool | listed only; existing `audio-noise-reduce` covers that family |
| Silence removal/shortening | out-of-model for this tool | listed only; gate preserves duration |
| Side-chain/key input and multichannel linking | out-of-model | listed; over-complex for this browser tool |
| Live preview meters/waveform | out-of-model | listed; generator runtime has no bespoke metering UI |

## Design decisions

- Use ffmpeg `agate` instead of custom DSP so CLI/page/chat all share proven behavior.
- Present threshold and reduction in dB, then convert to the linear `agate` scalars in core.
- Reject `reduction=0` because it is an advertised no-op.
- Keep defaults conservative for voice cleanup: `threshold=-35`, `reduction=30`, `ratio=2`, `attack=10`, `release=250`, `detection=rms`, `format=mp3`.
- State clearly that this is not spectral noise reduction and does not shorten audio.

## Verification plan

Unit tests cover argv order, dB-to-linear conversion, every format codec, detection parsing, boundary values, range errors, no-op rejection, and schema drift. Page tests upload real fixture audio, decode generated output, verify duration preservation, deep-link controls, alternate format, enum choices, and cap/boundary values.

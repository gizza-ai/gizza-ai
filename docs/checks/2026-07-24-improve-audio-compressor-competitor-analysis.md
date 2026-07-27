# Audio Dynamic-Range Compressor — Competitor Analysis (2026-07-24)

Tool: `audio-compressor` — applies dynamic-range compression to an audio file
via ffmpeg `acompressor`, exposing the four classic controls (threshold, ratio,
attack, release) plus make-up gain. Audio in, audio out (mp3/wav/ogg/flac/m4a);
album art dropped, samples re-encoded. This is loudness/dynamics compression,
distinct from the file-size `audio-compress` tool.

## Competitor comparison

| Tool | How it's framed | Manual T/R/A/R knobs? | Make-up gain | Underlying approach | In/out | Local vs cloud |
|------|-----------------|-----------------------|--------------|---------------------|--------|----------------|
| **Audio compressor sites (bulk "online audio compressor" pages)** | Mostly *file-size* shrink; a few offer a "dynamics" mode | Rarely; a single "amount" slider at most | Auto | Bitrate re-encode; dynamics is secondary | Audio | Cloud upload typical |
| **Auphonic** | "Adaptive Leveler" + loudness targets | No raw T/R/A/R — Dynamic Range (LU) + compressor Soft/Medium/Hard | Automatic | ML/statistical adaptive leveling + loudness norm | Audio/video | Cloud |
| **Adobe Podcast (Enhance)** | One-click "enhance speech" | No | Automatic | ML speech re-synthesis (bundles compression, denoise, EQ) | Audio | Cloud |
| **Online DAW-style editors (e.g. TwistedWave, AudioMass)** | In-browser waveform editor with an effects menu | Yes — a compressor effect with threshold/ratio/attack/release | Yes (gain) | Real DSP compressor on the decoded buffer | Audio | Mixed (AudioMass local; TwistedWave cloud) |
| **Desktop DAWs / plugins (Audacity, Reaper ReaComp)** | Full compressor effect | Yes — full T/R/A/R + knee | Yes | Native DSP | Audio | Local install |

## Table-stakes features (most compressor UIs offer)

- **The four canonical controls** — threshold, ratio, attack, release — for any
  tool that markets itself as a real compressor (DAWs, plugins, waveform
  editors). Consumer "enhance" tools hide them behind a single slider.
- **Make-up / output gain** so the result isn't quieter after compression.
- **Sensible defaults** that do something useful on first run (a firm 4:1 at a
  moderate threshold), so a user can just hit go.
- **Broad input format support**, audio re-encoded to a common output format.
- **Output naming** derived from the source (`… -compressed`).

## Params / defaults / UX per tool

- **ffmpeg `acompressor` (our engine)** — threshold, ratio, attack, release,
  makeup, knee, plus detection/link/mix. Defaults ≈ threshold 0.125 (−18 dBFS),
  ratio 2, attack 20 ms, release 250 ms, makeup 1, knee 2.83.
- **Waveform editors (TwistedWave/AudioMass)** — expose threshold/ratio/attack/
  release/gain in an effect dialog; outcome shown on the waveform.
- **Auphonic / Adobe** — hide DSP entirely behind targets/one-click strength.
- **Audacity Compressor** — threshold, noise floor, ratio, attack, release, plus
  "make-up gain to 0 dB" and "compress based on peaks" toggles.

Framing split: pro/editor tools expose the raw knobs; consumer "enhance" tools
sell the *outcome* and expose at most one slider.

## In-model vs out-of-model

**IN-MODEL (shipped):**
- **The four canonical controls + make-up gain** — threshold (−60…0 dB), ratio
  (1…20), attack (0.01…2000 ms), release (0.01…9000 ms), makeup (0…24 dB).
  This is the table-stakes "real compressor" surface and mirrors what
  waveform-editor and DAW compressors expose.
- **Sensible defaults** (threshold −20, ratio 4, attack 20, release 250, makeup
  0) so first-run does a firm, even compression; a page placeholder shows each.
- **Pure no-op rejected** (ratio 1 with 0 dB make-up gain) with a message that
  points at raising the ratio or adding make-up gain.
- **Format choice** (mp3/wav/ogg/flac/m4a) matching the rest of the audio
  family; `-compressed` output naming; album art dropped.
- **Fully local** in-browser processing — a privacy edge over the cloud-upload
  competitors, and free.

**OUT-OF-MODEL (not built):**
- **Adaptive/segment-aware leveling** that distinguishes speech vs music
  (Auphonic, Adobe) — needs content analysis, a different capability class.
- **Loudness-target normalization** to a specific LUFS — that's the separate
  `audio-normalize` (`loudnorm`) tool; kept distinct so each stays focused.
- **File-size compression** — that's the separate `audio-compress` (bitrate)
  tool; the page and copy explicitly disambiguate the two.
- **ML speech enhancement / denoise / re-synthesis** (Adobe Enhance).
- **Knee, sidechain/detection, stereo-link, wet/dry mix** — `acompressor`
  supports these, but they're advanced knobs most consumers never touch; left
  out to keep the surface to the five controls that matter.
- **Named presets (light/medium/heavy)** — the raw controls plus documented
  starting points cover this; the preset UX lives in
  `video-audio-compress-dynamics` and `audio-effects-rack`.
- **Batch / multi-file**, waveform-region editing, cloud project workflows.

**Decision:** ship the five canonical controls (threshold/ratio/attack/release/
make-up gain) with proven defaults and a clear no-op guard — the exact
table-stakes surface of a real compressor, done locally and free. Loudness
targeting and file-size shrink are intentionally kept as separate, focused tools
rather than folded in here.

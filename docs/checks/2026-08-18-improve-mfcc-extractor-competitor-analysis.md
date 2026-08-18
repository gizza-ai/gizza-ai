# mfcc-extractor — competitor analysis (2026-08-18)

Scan run **before** implementing, per `.claude/skills/create-next-tool`. Everything below is a
paraphrased summary of publicly documented function signatures and defaults. **No competitor copy,
branding or trademarks are reproduced or reused** anywhere in this repo.

## Scope

Backlog row: `mfcc-extractor` — "Extracts MFCC coefficients from speech or audio and returns them as
a CSV matrix." (type hint `ffmpeg`; **reclassified `pure`** — symphonia already decodes every
container we care about under wasmi, and the whole MFCC pipeline is arithmetic. Pure means it also
runs on the chat Service Worker backend, strictly better than an ffmpeg-only block.)

## Competitors reviewed (top 3 reachable reference implementations)

There is no meaningful "paste a file, get MFCCs" web tool ecosystem — MFCC extraction is dominated
by three library implementations that everyone's numbers are compared against. Those are the real
competitors: if our matrix doesn't line up with one of them, it is wrong.

| # | Implementation | What it is | Reachable |
|---|---|---|---|
| 1 | librosa `feature.mfcc` (0.11) | The de-facto Python reference; mel-spectrogram → dB → DCT-II | yes |
| 2 | `python_speech_features` `mfcc()` | The classic speech/ASR-shaped extractor (ms framing, HTK mel, pre-emphasis, cepstral lifter, energy-in-C0) | yes |
| 3 | torchaudio `transforms.MFCC` | The PyTorch transform; deliberately librosa-aligned, wraps MelSpectrogram + DCT | yes |

## Table stakes observed, and where each one landed

Defaults in brackets are the competitor's; ours are in the last column.

| # | Capability / param | Seen in | Fit | Our param (default) |
|---|---|---|---|---|
| 1 | Number of cepstral coefficients [13 / 20 / 40] | all 3 | **in-model** | `n_mfcc` (13) |
| 2 | Mel filterbank size [26 / 40 / 128] | all 3 | **in-model** | `n_mels` (26) |
| 3 | Analysis frame length, ms [25 ms] | 2 (librosa/torchaudio express it as `n_fft` samples) | **in-model** | `frame_ms` (25) |
| 4 | Frame hop / step, ms [10 ms] | all 3 | **in-model** | `hop_ms` (10) |
| 5 | Low frequency bound [0 Hz] | all 3 | **in-model** | `fmin` (0) |
| 6 | High frequency bound [Nyquist] | all 3 | **in-model** | `fmax` (0 = Nyquist) |
| 7 | Pre-emphasis filter [0.97] | 2 | **in-model** | `preemphasis` (0.97) |
| 8 | Cepstral (sinusoidal) liftering [22] | 2 (librosa `lifter`, off by default) | **in-model** | `lifter` (22) |
| 9 | Analysis window function [Hamming / Hann] | all 3 | **in-model** | `window` (hamming; hann/blackman/rectangular) |
| 10 | Mel scale formula: HTK vs Slaney [both shipped] | 1, 2 | **in-model** | `mel_scale` (htk; slaney adds librosa's area normalisation) |
| 11 | Replace C0 with log frame energy [on] | 2 | **in-model** | `append_energy` (true) |
| 12 | Delta / delta-delta features | 2 (`delta()`), 3 (`ComputeDeltas`) | **in-model** | `deltas` (none / delta / delta_delta) |
| 13 | Fixed analysis sample rate [16 kHz / 22.05 kHz] | all 3 (resample on load) | **in-model** | `resample_hz` (0 = native) |
| 14 | Numeric matrix out, frames × coefficients | all 3 | **in-model** | `output` (csv / tsv / json) |
| 15 | Frame timestamps alongside the matrix | 1 (`frames_to_time` helper) | **in-model** | `include_time` (true) |
| 16 | Rounding / print precision for text export | — (NumPy print options) | **in-model** | `decimals` (6) |
| 17 | Multi-channel input handling | 1 (multi-channel arrays), 2/3 (mono only) | **in-model** (documented) | automatic mono downmix |
| 18 | Accepts many containers/codecs | all 3 (via soundfile/audioread/torchcodec) | **in-model** | symphonia: WAV/AIFF/CAF/FLAC/MP3/OGG/MP4-M4A/MKV-WebM/AAC-ADTS |

### Out-of-model (documented, deliberately NOT built)

Each of these is a real competitor capability that does not fit a single stateless
paste-bytes-in / text-out block; they are listed here rather than silently dropped.

- **DCT types 1 and 3, and `norm=None`.** We implement DCT-II with orthonormal scaling only —
  that is the default in all three competitors. Types 1/3 exist in librosa/torchaudio for
  round-tripping, not for feature extraction.
- **`center=True` reflect-padding framing** (librosa/torchaudio pad the signal so frame *t* is
  centred on sample *t·hop*). We frame from sample 0 with no padding, the speech-toolkit
  convention; this changes the frame count and shifts alignment by half a frame, so it is called
  out explicitly in the page copy rather than half-supported.
- **dB-scaled (`power_to_db`, `ref=max`, `top_db=80`) log stage.** We always take the natural log
  of the mel energies. librosa's dB scaling differs by a constant factor of 10/ln 10 ≈ 4.343 plus a
  per-frame max reference and an 80 dB floor; documented in the FAQ instead of adding a mode.
- **CMVN / per-utterance cepstral mean-variance normalisation.** A separate post-processing step in
  every toolkit, not part of the `mfcc()` call.
- **Filterbank-only (`logfbank`) and spectral-contrast/chroma sibling features.** Different tools.
- **Batched directory/corpus extraction and `.npy` output.** This block is one file, one matrix.

## Design decisions taken from the scan

1. **Default to the speech convention, not the music one.** The backlog description says "speech or
   audio", and the pss defaults (25 ms / 10 ms / 13 coefficients / 26 filters / 0.97 pre-emphasis /
   lifter 22 / HTK mel / energy in C0) are what ASR feature pipelines actually use. librosa's
   defaults (20 coefficients, 128 mel bands, 2048-sample frames, no lifter, Slaney mel) are tuned
   for music analysis and are reachable by changing five fields.
2. **Frames in milliseconds, not samples.** Two of the three competitors express framing in samples
   (`n_fft`, `hop_length`), which is only meaningful once you know the sample rate. Milliseconds are
   sample-rate-independent and survive the `resample_hz` knob; the resolved sample counts and the
   derived FFT size are reported back in the JSON metadata so the mapping is never hidden.
3. **FFT size is derived, not exposed.** All three competitors let you set it, but the only sane
   value is the next power of two at or above the frame length — which is what we compute. Exposing
   it invites a broken zero-padding configuration for no capability gain; the resolved value is
   reported in the JSON output.
4. **Preset chips instead of a preset dropdown.** Competitors ship their conventions as *defaults*
   of different functions; we ship them as `[[example]]` chips on the page ("speech / ASR 13×26",
   "librosa-style 20×40", "+ delta & delta-delta", "wide-band music") so one click reproduces each
   house style.
5. **Sliders for the continuous knobs.** `n_mfcc`, `n_mels`, `frame_ms`, `hop_ms`, `preemphasis` and
   `lifter` are all bounded scalars that users sweep rather than type — declarative `kind = "slider"`
   controls, with the canonical number box kept as the source of truth.
6. **Honest caps.** 24 MiB of pasted bytes, 4,000,000 mono samples analysed and 200,000 output
   frames, each reported (and truncation flagged) rather than silently applied.

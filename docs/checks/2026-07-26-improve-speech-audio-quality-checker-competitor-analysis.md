# Competitor analysis — speech-audio-quality-checker (2026-07-26)

Pre-flights a recording for ASR (speech-to-text) by decoding the actual audio and
checking sample rate, channel count, estimated SNR, and clipping against
transcription-readiness thresholds. All findings below are **paraphrased** — no
competitor copy, branding, or trademarks are reproduced.

## Sources skimmed

1. **SoapBox Labs — Audio Quality Feature** (docs.soapboxlabs.com) — a commercial
   ASR provider's per-request audio-quality signal.
2. **OpenAI Whisper optimal-input guides** (community gist + saytowords.com) — the
   de-facto reference for "what audio does a modern ASR model want".
3. **Speech-quality / clipping-detection literature** (ScienceDirect clipping-detection
   paper; futurebeeai SNR knowledge hub; multi-dimensional speech-quality assessment,
   arXiv 2309.07385) — how SNR and clipping are defined and thresholded for speech.

Unreachable/irrelevant results (patents, TTS-dataset papers) were skipped in favor of
the reachable, on-topic references above.

## Table-stakes: metrics + thresholds

| Metric | What competitors do | Our decision |
|---|---|---|
| **Sample rate** | Whisper resamples everything to **16 kHz**; higher rates give no accuracy gain, only size. 8 kHz (telephone) works but is marginal. | Report the file's rate; PASS if ≥ target (default **16000**), WARN if between 8 kHz and target (upsampling won't add detail), FAIL if < 8 kHz. Target is a param. |
| **Channels** | Mono strongly preferred; Whisper/most ASR downmix to mono internally. | Report channel count; PASS if mono, WARN if >1 (will be downmixed — extra size, no accuracy gain). |
| **SNR** | SoapBox reports SNR in dB as the single headline quality number. Their bands: <0 discard, 0–9 very noisy, 10–19 noisy/maybe, 20–39 clean, 40+ very clean. FutureBee: 20–30 dB "good" for ASR. | Estimate SNR (percentile method, documented as an estimate) in dB; PASS if ≥ `min_snr_db` (default **20**), WARN 10–20, FAIL < 10. Threshold is a param. |
| **Clipping** | Literature: a sample is "clipped" when within ~5% of full scale; sustained clipped runs are the real distortion. | Count samples with \|amp\| ≥ 0.99 FS and sustained clipped runs; report clipped %; PASS if ≤ `max_clipping_pct` (default **1.0**), else WARN/FAIL. Threshold is a param. |
| **Loudness / level** | Mentioned as a quality dimension (too-quiet audio measures noise). | Report peak dBFS and RMS dBFS as context; flag "very quiet" (peak < −30 dBFS) in the report. |
| **Bit depth / format** | 16-bit PCM WAV/FLAC recommended for fidelity; lossy is tolerated. | Decode wav/flac/mp3/ogg/m4a/aac; report codec + bit depth where known. |

## In-model vs out-of-model

- **In-model (built):** decode real PCM (symphonia, wasm-proven), measure sample rate,
  channels, duration, peak/RMS dBFS, percentile-SNR estimate, clipping % + runs;
  per-check PASS/WARN/FAIL against configurable thresholds; overall readiness verdict;
  human `report` and machine `json` output; base64/hex byte input (pure-tool paste
  surface, no server).
- **Out-of-model (listed, not built):** server-side batch scoring, cloud API integration,
  full VAD-based speech/non-speech SNR, PESQ/POLQA/DNSMOS perceptual MOS models (need an
  ML model — out of a pure-Rust wasm tool), and file-upload of arbitrarily large
  recordings (pure tools take pasted bytes; short representative clips are the fit).

## UX controls competitors expose

- Numeric SNR readout → we show it plus a plain-language band.
- Pass/fail per dimension → we render per-check status + an overall verdict.
- Configurable target rate / thresholds → exposed as params with ASR-sane defaults.
- Preset examples → page ships `[[example]]` chips with a real embedded WAV clip.

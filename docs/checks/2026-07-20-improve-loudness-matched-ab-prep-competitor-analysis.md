# loudness-matched-ab-prep — competitor analysis (2026-07-20)

New-tool build. One WebSearch ("loudness match two audio files LUFS gain match
A/B comparison tool"), skimmed three reachable competitors (Adobe Audition's
Match Loudness page timed out and was replaced by ffmpeg-normalize; paraphrased
— no copy/branding taken):

1. **WU Tools Audio Compare** (browser, local) — two uploads (mp3/wav/ogg/
   aac/m4a/flac/opus); reports integrated LUFS (BS.1770 K-weighted, gated),
   true peak dBTP (4x oversampled), sample peak, RMS, spectral stats
   (centroid/rolloff/flatness/ZCR); shows the loudness difference in LU and
   which file is louder; A/B playback toggle with keyboard shortcuts; platform
   target dropdown (streaming -14/-1, YouTube, Apple, EBU R128). Does NOT
   export gain-matched copies — the user is told the LU trim to apply by hand.
2. **ffmpeg-normalize** (open-source CLI) — EBU R128 two-pass loudness
   normalization; target level (-t, default -23 LUFS; presets podcast -16,
   music, streaming-video), true-peak ceiling (-tp), RMS and peak modes,
   `--lower-only` (never boost), `--batch` (preserve relative loudness across
   files), any output codec via ffmpeg.
3. **MeterPlugs Perception AB** (commercial plugin — the category reference) —
   measures the loudness of the pre- and post-chain signals and applies the
   compensating gain to one side ("Match Level"), so A/B toggling is
   loudness-fair; realtime, with latency compensation.

## Table stakes → decision

| Capability | Tag | Where |
|---|---|---|
| Integrated LUFS per file (ITU-R BS.1770 K-weighting + gating) | in-model | ebur128 crate; reported per file |
| Loudness difference in LU + which file is louder | in-model | report + for_llm summary |
| Gain-matched output copies (the actual prep — none of the browser tools export) | in-model | zip with a-matched.wav + b-matched.wav + report.txt |
| Never-boost matching (ffmpeg-normalize `--lower-only`, standard A/B practice) | in-model | `mode=quieter` (default): louder file attenuated to the quieter one |
| Match both to an explicit LUFS target (ffmpeg-normalize `-t`) | in-model | `mode=target` + `target_lufs` (platform values in the describe) |
| True peak (dBTP) before/after gain + clipping warning | in-model | ebur128 true-peak mode; post-gain TP = TP + gain (linear) |
| RMS level per file | in-model | computed on decoded PCM, in the report |
| Loudness range (LRA) per file | in-model | ebur128 LRA mode, in the report |
| Common input formats (wav/flac/mp3/ogg/m4a) | in-model | symphonia pure-Rust decode (Vorbis for ogg, AAC-LC/ALAC for m4a; no opus) |
| Lossless output (fair comparison path) | in-model | WAV out; `bit_depth` 16/24/32f (both files re-encoded identically) |
| True-peak LIMITER on boost (ffmpeg-normalize applies loudnorm's limiter) | out-of-model | linear gain only; int output hard-clamps + reports clipped-sample count, warning suggests 32f/lower target |
| A/B playback UI (toggle, speed, loop, keyboard shortcuts) | out-of-model | two-file input has no page surface (single-upload page driver); chat+CLI only — outputs feed any player/ABX tool |
| Spectral stats (centroid/rolloff/flatness/ZCR) | out-of-model | analyzer identity, not A/B prep; deliberately excluded |
| Sample-rate conversion / codec output beyond WAV | out-of-model | no resampling (each output keeps its source rate — loudness is rate-independent); WAV only |
| Batch (>2 files) relative-loudness preservation | out-of-model | exactly two files is the A/B contract |

## Design notes

- **No page surface**: two media inputs don't fit the single-upload page
  driver (same constraint as the skiplisted add-audio-to-video family), but
  unlike those, the processing here is PURE Rust (symphonia decode + ebur128 +
  hound), so chat + CLI are fully viable — the image-collage
  `Param::source_list` + no-page pattern.
- Both files always pass through the identical decode → gain → re-encode WAV
  pipeline (even the unchanged one at unity gain), so the comparison path
  itself is codec-fair — the point Perception AB makes about matched signal
  paths.
- Envelope model caps the output: 10 MiB per input file, 32 MiB zip out,
  max 300 s decoded per file, min 3 s (R128 gating needs blocks), mono/stereo
  only. Practical capacity ≈ a 45–90 s section per file at 44.1 kHz stereo —
  stated in the tool description; pairs with trim-audio for longer masters.
- Post-gain true peak is exact, not re-measured: linear gain scales the
  oversampled peak linearly, so TP_after = TP_before + gain_dB.
- Silent/ungateable input (integrated loudness = -inf) is a hard error naming
  the offending file, not a NaN gain.

## Verified

- `cargo test --workspace`: 15 tests — 12 core unit (quieter/target modes with
  re-measure of the zip contents, all three bit depths write the advertised
  WAV format, clip clamp+count on boost, 32f over-preservation, mono+stereo
  pair, output-cap guidance, silent / too-short / garbage errors, parser
  rejects, already-matched) + 2 format integration tests (committed 3.5 s
  flac/ogg/m4a fixtures — no ffmpeg, no network) + 1 block schema drift-guard.
- Block wasm + CLI (real wasm instantiation of symphonia+ebur128+hound+zip
  under the wasmi runtime): default quieter/16 run on two public kozco.com
  WAVs (48 kHz + 44.1 kHz pair) — summary deterministic, and ffmpeg's own
  ebur128 filter independently measures a-matched.wav at -16.4 LUFS (ours:
  -16.43); mode=target target_lufs=-14 bit_depth=24 → ffmpeg measures BOTH
  outputs at -14.0 LUFS, ffprobe confirms pcm_s24le; mp3 input + 32f →
  pcm_f32le confirmed (mp3 LUFS -16.39 vs the same audio's WAV -16.43 —
  codec-consistent); error matrix: 1 file, 3 files, target_lufs=-40, unknown
  mode, HTTP 404 — all graceful exit 1 with guiding messages; long m4a (105 s)
  → clean PCM-cap error (this case originally OOM-trapped the 64 MiB sandbox
  via Vec doubling — fixed with bounded 4 MiB-step growth in decode).
- flac-over-HTTP with `application/octet-stream` content-type is rejected by
  the shared resolve_source MIME class check (family-standard behavior; the
  error is graceful and self-explanatory).
- `wafer build` validation: fails on this box with the stale-CLI
  `__wafer_info()` symptom — identically for already-shipped green blocks
  (har-request-extract, data-anonymizer), i.e. environmental; wafer CLI is
  optional and CI never uses it.
- No Playwright/page checks: no page surface (two-input tool, documented
  above). Generator + per-slug strict hygiene gate both pass.

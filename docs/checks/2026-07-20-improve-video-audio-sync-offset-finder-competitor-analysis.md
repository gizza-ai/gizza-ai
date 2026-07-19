# video-audio-sync-offset-finder — competitor analysis (2026-07-20)

New-tool build. One WebSearch ("find audio time offset between two videos
cross-correlation sync tool align recordings"), skimmed three reachable
competitors (paraphrased — no copy/branding taken):

1. **BBC audio-offset-finder** (open-source CLI, the category reference) —
   finds the offset of one audio/video file within another via
   cross-correlation of standardized MFCCs; ffmpeg-decodes any input; `--sr`
   resample rate (default 8000 Hz), `--trim` (analyze only the first n
   seconds), `--resolution` (MFCC hop), `--json`; outputs the offset in
   seconds plus a **standard score** (z-score of the correlation peak —
   ≥ 10 reliable, < 5 verify manually); accuracy ~0.01 s; warns that highly
   repetitive audio confuses it.
2. **SyncSink** (open-source GUI, Java/AGPL) — drag-and-drop N recordings of
   the same event; **two-stage**: acoustic-fingerprint rough offset (~8 ms)
   then crosscovariance refine to sample accuracy; emits ready-to-run ffmpeg
   commands that actually align the files.
3. **AudAlign** (open-source Python) — fingerprinting + cross-correlation +
   spectrogram alignment; `max_lags` bound on the searched offset, `locality`,
   noise-reduction knob; outputs offsets plus a 1–10 confidence ranking; can
   write silence-padded aligned copies.

## Table stakes → decision

| Capability | Tag | Where |
|---|---|---|
| Offset in seconds between two recordings (video or audio, any mix) | in-model | core cross-correlation; `offset_seconds` + sign convention stated everywhere |
| Reads audio out of video containers | in-model | symphonia isomp4/mkv demux (video tracks skipped); MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, AAC-ADTS |
| Standard-score confidence (BBC's signature output, ≥10 / 5–10 / <5 bands) | in-model | per-lag variance-normalized z-score (`score`) + `confidence` label with the same band semantics |
| Two-stage coarse→refine (SyncSink's ~8 ms then sample-level) | in-model | 100 Hz novelty-envelope NCC coarse pass → 8 kHz waveform NCC fine pass (±0.6 s, parabolic sub-sample interp, ~1 ms) |
| Analyze-only-first-n-seconds (BBC `--trim`) | in-model | `analyze_seconds` (default 120, 5–240) |
| Bounded offset search (AudAlign `max_lags`) | in-model | `max_offset` (default 0 = unlimited) |
| Actionable alignment output (SyncSink's ffmpeg commands) | in-model | `align_hint` text with concrete ffmpeg `-ss` / `-itsoffset` values (report-only — see out-of-model) |
| Confidence ranking (AudAlign 1–10) | in-model | `score` + `confidence` (high/medium/low/none) + targeted `warning`s |
| 8 kHz analysis rate (BBC `--sr` default) | in-model (fixed) | hard-wired to BBC's default; not a knob — fine-pass refinement supplies the precision a higher rate would |
| Repetitive-audio caveat (BBC README warning) | in-model | multi-candidate fine verification (top-5 coarse peaks re-checked by waveform) + `at_search_edge`/low-score warnings |
| ffmpeg-universal input decode (Opus, AC-3, DTS, HE-AAC, AVI…) | out-of-model | pure-Rust symphonia subset only; unsupported codecs error with the full supported list |
| Writing ALIGNED media outputs (SyncSink re-encode, AudAlign padded copies) | out-of-model | report + hint only; producing a re-muxed pair needs multi-input ffmpeg output (the skiplisted video-concat class) |
| N-file batch alignment against one reference | out-of-model | exactly two files per run (the BBC pairwise model); run repeatedly for more angles |
| Acoustic fingerprinting / MFCC features | out-of-model | envelope+waveform NCC instead (no DSP-feature stack in wasm); the standard score + waveform lock cover the same confidence need |
| GUI with plots (SyncSink timebox, BBC `--show-plot`) | out-of-model | no page surface — two media inputs do not fit the single-upload page driver |
| Noise-reduction preprocessing knob (AudAlign `prop_decrease`) | out-of-model | novelty envelope + NCC are already level/noise-tolerant; no extra knob |

## Design notes

- **No page surface**: two media inputs don't fit the single-upload page
  driver, and this is pure Rust (multi-input ffmpeg stays skiplisted — see the
  video-mux-external-audio class), so surfaces are chat + CLI — the
  loudness-matched-ab-prep / image-collage `Param::source_list` no-page
  pattern.
- **Decode is memory-shaped for the 64 MiB sandbox**: symphonia decodes
  packet-by-packet, downmixes to mono and integrate-and-dump resamples to
  8 kHz on the fly — native-rate PCM is never held (240 s × 2 files ≈ 15 MiB
  of analysis signal; FFT buffers ≤ ~14 MiB transient).
- **Coarse pass statistics**: raw per-lag NCC values are not comparable across
  lags (short overlaps are noisier), so both peak-picking and the z-score use
  NCC × sqrt(overlap) — without this, unrelated recordings scored up to ~5.6
  and landed in "medium"; with it they sit ≈ 4 ("none" unless the waveform
  locks). The envelope is the FIRST DIFFERENCE of 100 Hz log-energy
  (novelty): a smooth envelope correlates over a wide lag range and drowned
  the true peak's z-score (~3.6 for a perfect match before the change).
- **Multi-candidate verification**: the top-5 coarse peaks (≥ 0.5 s apart) are
  each re-checked by the waveform fine pass and the best lock wins — this is
  what recovers heavy-noise cases where the best envelope peak is wrong.
- **Sign convention** is stated in the schema, description and output:
  offset > 0 ⇔ file 2 starts that many seconds AFTER file 1; `align_hint`
  spells out which file to trim (`-ss`) or delay (`-itsoffset`).
- **Polarity inversion** (one chain flips the signal) is detected from a
  negative waveform peak and flagged, not treated as a failure.
- Real-world check: kozco piano2.wav vs piano2-CoolEdit.mp3 (same recording,
  different codec) → −0.025 s at correlation 0.999 — exactly the MP3
  encoder-delay ballpark, waveform-locked.

## Verified

- `cargo test --workspace`: 24 tests — 17 core unit (FFT round-trip, FFT-vs-
  direct xcorr equivalence on non-power-of-two lengths, ±5 s offsets to ~1 ms,
  identical files at 0, 0 dB-SNR + gain-mismatch robustness, polarity
  inversion, unrelated-noise → "none", max_offset restriction, too-short /
  silent / garbage rejects, resampler down/up/cap, parameter ranges,
  confidence bands) + 6 fixture integration (committed a.mp4/b.mp4 AAC+H.264,
  c.wav 22.05 kHz, d.webm Vorbis+VP8, e-video-only.mp4 — cuts of one
  deterministic noise master at known offsets +2.5/+1.0/+4.0 s, same-codec
  tolerance 30 ms, cross-codec 60–80 ms, video-only → clear error) + 1 block
  schema drift-guard.
- Block wasm + CLI (real wasm instantiation of symphonia + the hand-rolled
  FFT under the wasmi runtime): same public MP4 twice → offset exactly 0.0,
  score 20.1, correlation 1.0, "high", byte-identical across two runs
  (exact-output case); wav-vs-mp3 same recording → −0.025 s waveform-locked;
  unrelated organ/piano pair → envelope-only "medium" with spot-check warning
  (both files share an attack-at-start structure; BBC's 5–10 "verify" band);
  video+audio mixed inputs OK; boundary matrix analyze_seconds ∈ {5, 240},
  max_offset ∈ {2, 240} all run; errors all graceful exit 1 with guiding
  messages: video-without-audio (names the file, lists supported codecs),
  1 file, analyze_seconds=3, max_offset=500, HTTP 403/404 fetch.
- Generator run: repo renders, pageless tool correctly absent from pkg/tools/.
- `sync-tool-manifest.py --check` + per-slug strict hygiene: clean.
- No Playwright/page checks: no page surface (two-input tool, documented
  above). Lockfile pinned at wafer-run-pin.txt rev (scaffold auto-pin).

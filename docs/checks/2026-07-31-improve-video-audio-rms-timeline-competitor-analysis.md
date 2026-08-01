# video-audio-rms-timeline — competitor analysis (2026-07-31)

**Tool:** Extract windowed RMS and peak audio levels from a video's audio track
into a CSV/JSON time series.

**Classification decision:** the backlog `type_hint` was `ffmpeg`, but this is a
**pure** (symphonia) tool, not an ffmpeg-runtime tool. The deliverable is a
**text** time series (CSV). The gizza ffmpeg page runtime hard-requires a
media output element (`#tool-output-media` / `#tool-output-download`, emitted
only for `format = image|video|audio`) and renders exactly one output file as
`<img>/<video>/<audio>`; a `runtime=ffmpeg` + `format=text` page hits the
`"ffmpeg runtime needs an image/video/audio output"` guard (this is why
`video-frame-fingerprint` / `video-match-cut-finder` were skiplisted). A CSV
tool therefore cannot use the ffmpeg runtime. Instead it is built exactly like
`speech-audio-quality-checker` (pure symphonia decode of pasted base64/hex
bytes → text report) and `video-audio-sync-offset-finder` (pure symphonia decode
of a **video's** audio track). symphonia decodes the audio track out of MP4/MKV
video containers, so the tool is fully in-model as a pure block on all three
surfaces (chat + CLI via url/base64, page via a base64 text field).

## Competitors surveyed

1. **ffmpeg `astats` + `ametadata=print`** — the canonical CLI approach:
   `astats=metadata=1:reset=1` with `asetnsamples=N` chunks emits per-chunk
   `lavfi.astats.Overall.RMS_level` and `Peak_level` in **dBFS**, written to a
   log via `ametadata=print:file=`. Users then post-process the log into CSV.
   Table stakes: chunk/window size, RMS + peak, dBFS, per-frame timestamps.
2. **librosa (`librosa.feature.rms`)** — Python: windowed RMS with
   `frame_length` + `hop_length` (overlapping frames), returns a **linear**
   amplitude array; `librosa.frames_to_time()` converts frame index → seconds;
   users export to CSV with pandas. Table stakes: frame length, hop/overlap,
   linear unit, per-frame time.
3. **MAZTR online Audio File Analyzer** — free, no account; measures peak and
   RMS over a short (~50 ms) window and shows peak/trough. Table stakes: short
   RMS window, peak + RMS, browser-based/no-upload feel.
4. **VU-meter desktop apps (e.g. the "VU Meter" Windows app)** — live peak dB,
   RMS dB, configurable window / hold. Table stakes: peak dB, RMS dB,
   configurable window.
5. **mediadeepa / ffmpeg-normalize report tooling** — batch level/loudness
   reports over a media file (built on ffmpeg astats/ebur128). Table stakes:
   works directly on a video file; structured (CSV/JSON) export.

## Table-stakes → decision

| Capability | Competitor(s) | In model? | Where |
|---|---|---|---|
| Windowed RMS per frame | all | yes | `window_ms` + `rms` column |
| Windowed peak per frame | astats, VU, MAZTR | yes | `peak` column |
| Configurable window length | astats (`asetnsamples`), librosa (`frame_length`), MAZTR | yes | `window_ms` (1–60000) |
| Hop / overlapping frames | librosa (`hop_length`) | yes | `hop_ms` (0 = non-overlapping) |
| dBFS unit | astats, VU | yes | `unit=dbfs` (default) |
| Linear amplitude unit | librosa | yes | `unit=linear` |
| Per-frame timestamps | librosa `frames_to_time`, astats | yes | `start_s` / `end_s` columns |
| CSV export | astats(post), librosa(pandas) | yes | `output=csv` (default) |
| JSON export | mediadeepa | yes | `output=json` (+ stream metadata) |
| Works on a **video** file's audio | astats, mediadeepa | yes | symphonia decodes the audio track of MP4/MKV/WebM |
| No-upload / browser | MAZTR | yes | page decodes in wasm |

## Out of model / intentionally not built (listed, not built)

- **LUFS / EBU R128 integrated loudness** — a psychoacoustically-weighted,
  gated measure (K-weighting + gating), distinct from raw RMS. It is a different
  tool class (ffmpeg `ebur128`) and gizza already ships `loudness-spec-compliance`
  / `loudness-matched-ab-prep` for loudness. This tool deliberately reports raw
  RMS/peak, not LUFS.
- **Per-channel columns** — competitors that report L/R separately assume a
  fixed stereo layout. To keep the CSV portable across 1–8 channel files the
  tool measures the mono downmix; documented as a limitation.
- **Spectrogram-domain RMS** — librosa can compute RMS from an STFT magnitude
  spectrogram. Time-domain RMS over decoded samples is the standard and
  sufficient here; no FFT dependency added.
- **Charts / waveform image** — the deliverable is a machine-readable time
  series (CSV/JSON) for the user to chart themselves, not a rendered plot.
- **Opus / AC-3 / DTS decoding** — not supported by the pinned symphonia
  feature set; rejected with a clear message (same set as
  `video-audio-sync-offset-finder`).

## UX patterns adopted

- Enum controls rendered as `<select>` with friendly `[input.labels]`
  (Base64/Hex, dBFS/Linear, CSV/JSON).
- Number fields with placeholders (`window_ms` → 100, `hop_ms` → 0).
- Three preset **example chips** (100 ms CSV dBFS; overlapping 50 ms hop;
  JSON linear) prefilling a real bundled base64 tone — mirrors the
  frame/hop/unit presets competitors expose.
- `format = "text"` page with a Download link for the CSV/JSON result.

No competitor copy, branding, or trademarks were reproduced; all copy is
original and paraphrased.

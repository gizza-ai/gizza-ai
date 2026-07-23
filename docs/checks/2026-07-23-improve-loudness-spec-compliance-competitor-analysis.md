# loudness-spec-compliance — competitor analysis (2026-07-23)

Tool function: decode one audio file, measure ITU-R BS.1770 / EBU R128 **integrated
loudness (LUFS)**, **true peak (dBTP)** and **loudness range (LRA)**, then compare each
against a named delivery spec (EBU R128, ATSC A/85, streaming targets) and return a
**pass/fail per criterion** verdict. Paraphrased research only — no competitor copy,
branding, or trademarks reproduced.

## Competitors scanned

1. **loudcheck (PyPI, CLI)** — measures a media file with ffmpeg's `ebur128`/`loudnorm`
   and prints whether the file **passes the spec**; ships presets for **EBU R128** and
   **ATSC A/85**. Output is a per-criterion pass/fail. Closest in-shape competitor. In-model.
2. **Browser loudness meters (wutools / toolnar / editingtools loudness)** — measure full
   EBU R128 / BS.1770-4: **integrated, short-term, momentary** LUFS + **4× true peak (dBTP)**
   + **LRA**, all in-browser (WebAudio), then show a **green PASS / red FAIL** badge when
   integrated is within **±1 LU** of target and true peak is at/under the ceiling. Integrated
   / TP / LRA are in-model; short-term/momentary time-series and live metering are out-of-model
   (no page, single-shot compute).
3. **NUGEN / Emotion Systems / broadcast QC (verify/correct suites)** — professional file-based
   QC: measure integrated + TP + LRA against a house/broadcast profile, PASS/FAIL report, and
   *auto-correct* to spec. Measurement + verdict in-model; auto-correction is out-of-model here
   (that is what the existing `audio-normalize` / `loudness-matched-ab-prep` tools cover).
4. **MediaLive / cloud encoders (loudnorm target LKFS presets)** — apply a target LKFS during
   encode. Application, not verification — out of this tool's scope (measure + verdict only).

## Table-stakes → decision

| Table-stake (paraphrased)                          | In-model? | Where it lands |
|----------------------------------------------------|-----------|----------------|
| Named spec presets (EBU R128, ATSC A/85)           | yes       | `standard` enum |
| Streaming targets (Spotify/YouTube/Apple/Amazon)   | yes       | `standard` enum |
| Integrated loudness (LUFS) measure + target check  | yes       | `integrated_loudness` check |
| True peak (dBTP, 4× oversampled) measure + ceiling | yes       | `true_peak` check |
| Loudness range (LRA) measure                       | yes       | reported; checked only when a spec caps it |
| Pass/fail per criterion + overall verdict          | yes       | `checks[]` + overall `pass` |
| Configurable tolerance / custom spec               | yes       | `standard=custom` + `target_lufs`/`tolerance_lu`/`max_true_peak`/`max_lra` |
| Short-term / momentary time-series, live meter     | no        | out-of-model — single-shot compute, no page/streaming surface |
| Auto-correct / normalize to spec                   | no        | out-of-model — covered by `audio-normalize`, `normalize-peak`, `loudness-matched-ab-prep` |
| Dialog-gated loudness (Netflix -27 LKFS dialog)    | no        | out-of-model — needs a dialog-gating/VAD model; we measure program-gated BS.1770 only, so Netflix is deliberately omitted |

## Spec values used (from research)

- **EBU R128**: target **-23.0 LUFS**, permitted deviation **±1.0 LU** (file-based delivery can
  set ±0.5 via the `tolerance_lu` override), max true peak **-1.0 dBTP**. (tech.ebu.ch R128)
- **ATSC A/85** (US CALM Act): target **-24.0 LKFS**, tolerance **±2.0 LU** (dialnorm ±2), max
  true peak **-2.0 dBTP**.
- **Spotify / YouTube / Amazon Music / Tidal**: **-14.0 LUFS**; TP **-1.0 dBTP** (Amazon is the
  outlier at **-2.0 dBTP** — Echo/Alexa inter-sample-clip headroom).
- **Apple Music**: **-16.0 LUFS**, TP **-1.0 dBTP** (deliberate quieter outlier).

LKFS ≡ LUFS (identical unit, different name). All specs share the ITU-R BS.1770 measurement, so
one gated-integrated + 4× true-peak engine (the `ebur128` crate) serves every preset.

## UX patterns

Competitors present a green/red per-criterion verdict with the measured value, the limit, and
the delta. We mirror this as a JSON `checks[]` array (criterion, measured, limit, pass, delta
detail) plus an overall `pass` boolean and a one-line human summary for chat — the chat/CLI
analog of the badge grid. No page (single audio file into a pure-wasm block has no generic page
runtime — same shape as `loudness-matched-ab-prep` / `normalize-peak`).

Sources (paraphrased, not quoted): tech.ebu.ch R128; forasoft.com loudness normalization & 2026
platform-targets articles; pypi.org/project/loudcheck; production-expert.com loudness guide;
AES TD1008 streaming recommendation.

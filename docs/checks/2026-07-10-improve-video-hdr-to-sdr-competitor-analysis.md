# video-hdr-to-sdr — competitor analysis (2026-07-10)

Snapshot of how the top HDR→SDR conversion tools position themselves, and how gizza's
in-model (pure Rust + browser ffmpeg) tool compares. All observations are paraphrased;
no competitor copy, branding, or trademarks are reproduced.

## Landscape (paraphrased)

- **Desktop ffmpeg guides / power-user recipes.** The canonical approach documented
  across community wikis is exactly the `zscale`→`tonemap`→`zscale`→`yuv420p` chain
  (linearize the PQ/HLG transfer, gamut-map to BT.709, tone-map luminance, re-encode).
  They expose the raw `tonemap` operators (hable/mobius/reinhard/…) and the `npl`/`desat`
  knobs. Strength: full control. Weakness: command-line only; users must hand-assemble a
  fragile, order-sensitive filter string.
- **General "video converter" web/desktop apps.** Broad format converters that add an
  "HDR to SDR" checkbox. Strength: familiar UI, batch. Weakness: the tone-map is a hidden
  one-size preset with no curve choice; several upload the file to a server (privacy) or
  gate output behind a watermark/paywall; some produce visibly washed-out results because
  they skip proper linearization.
- **NLE / editor built-ins (color-management "HDR→SDR" transforms).** High-quality,
  per-shot control. Strength: best visual quality, scopes. Weakness: heavyweight install,
  paid, steep learning curve — overkill for "just make this clip look right."
- **Mobile "HDR fix" apps.** One-tap convenience. Weakness: opaque, often re-compress
  hard, ads, and upload to a cloud service.
- **Online "colorspace / Rec.2020→Rec.709" utilities.** Niche single-purpose pages.
  Strength: focused. Weakness: usually fixed operator + fixed output codec, and many are
  server-side uploads.

## Gap ranking (fit-to-model)

Closed in this build (in-model):

1. **Curve choice exposed, not hidden** — `tonemap` enum (hable default, plus mobius,
   reinhard, linear, clip) rendered as a labeled `<select>`, matching the power-user
   recipes but without the command line.
2. **The two knobs that actually matter** — `peak` (npl, nits, default 100) and `desat`
   (highlight desaturation 0-4, default 0), with placeholder-documented defaults.
3. **Correct, order-sensitive pipeline** — proper linearize→float-RGB→gamut→tonemap→
   re-encode chain, so results don't come out washed-out the way some one-preset
   converters do.
4. **Format + quality control** — MP4 (H.264/AAC) or WebM (VP9/Opus), quality 1-100→CRF.
5. **Privacy + no paywall/watermark** — runs entirely in-browser via WebAssembly ffmpeg;
   nothing is uploaded (a clear differentiator vs server-side converters).
6. **Preset chips + worked examples + FAQ** covering "why gray/dim", curve choice, peak.

Out of model (not built — would need capabilities gizza doesn't ship):

- Per-shot / keyframed color grading and scopes (waveform/vectorscope) — NLE territory.
- Batch/folder conversion and side-by-side HDR/SDR preview.
- 10-bit or HDR-metadata-preserving output (gizza targets 8-bit SDR BT.709 by design).
- GPU-accelerated or ML-based tone-mapping.

## Net

The gizza tool matches the power-user ffmpeg recipe's control surface (operator + npl +
desat + codec/quality) in a one-click, private, browser-only page — closing the
"washed-out hidden-preset" and "server upload / paywall" gaps that the mainstream web
converters have, while correctly scoping out heavyweight NLE-only features.

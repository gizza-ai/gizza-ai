# normalize-peak — competitor analysis (2026-07-23)

**Tool:** `normalize-peak` — "Scales a file so its loudest sample hits a target dBFS peak."
**Type:** pure (Rust→wasm audio decode + gain + WAV encode). Surfaces: **chat + CLI, no page**
(single audio file input + binary WAV output — the same chat+CLI / no-page shape as
`loudness-matched-ab-prep`, `normalize-image`, `image-collage`; the generic page runtime has no
upload-a-file-into-a-pure-wasm-block mode, and ffmpeg cannot exact-target a sample peak in one pass).

## What peak normalization is (and how it differs from siblings)

Peak normalization finds the **loudest single sample** in a file (its sample peak, in dBFS) and
applies **one constant gain** so that peak lands exactly on a chosen target level (e.g. −1 dBFS).
Nothing else about the waveform changes — the dynamics are untouched, only the overall level.

This is genuinely distinct from the two existing gizza audio-level tools:

| Tool | Method | Targets |
|------|--------|---------|
| **audio-normalize** (existing) | ffmpeg `loudnorm` (EBU R128) | perceptual **LUFS loudness** (TP capped at −1.5 dBTP); *dynamics-altering AGC*, not a plain scale |
| **audio-volume-adjust** (existing) | ffmpeg `volume=NdB` | a **fixed** dB/×factor gain the user names; does **not** measure the file |
| **normalize-peak** (this) | measure sample peak → scale to target dBFS | the file's **loudest sample** hits an exact dBFS peak |

So neither sibling does peak normalization: `audio-normalize` targets loudness (not peak) and
reshapes dynamics; `audio-volume-adjust` needs you to already know the gain. `normalize-peak`
*measures* the peak and computes the exact gain to hit a target — the single most-requested
"make it as loud as possible without clipping / to a fixed ceiling" level fix before export.
Not a duplicate.

## Competitors surveyed (top 5)

1. **Audacity — Effect ▸ Normalize.** The reference implementation. Controls:
   - "Normalize peak amplitude to" **dB** field, default **−1.0 dB**.
   - "Remove DC offset" checkbox (default **on**): subtract each channel's mean before scaling so
     the waveform is centred on 0.
   - "Normalize stereo channels independently" (default **off** = channels scaled by one common
     gain to preserve the L/R balance; on = each channel scaled to hit the target itself).
2. **Adobe Audition — Normalize (Peak).** "Normalize to __ %" or dBFS, with a "Normalize all
   channels equally" (= linked) toggle and DC-bias adjustment. Same three knobs as Audacity.
3. **SoX — `gain -n` / `--norm=dB`.** CLI peak normalizer: `sox in out gain -n -1` scales so the
   peak sits at −1 dBFS (default 0). Linked across channels ("balance" preserved) by default.
4. **ffmpeg-normalize `-nt peak -t <dBFS>`.** Python/ffmpeg wrapper; `--normalization-type peak`
   with a target level in dBFS (default 0 for peak mode). Two-pass (measure then apply) — which is
   why a single ffmpeg filter graph can't do it, and a pure-Rust measure-then-scale is the clean fit.
5. **mp3gain / wavegain-style "Maximize peak".** One-click "bring peak to 0 dBFS" utilities — the
   degenerate `target = 0` case of the same operation.

## Table-stakes → in-model / out-of-model

| Capability | Decision | In `normalize-peak` |
|------------|----------|---------------------|
| Target peak level in **dBFS**, default −1.0 | **in-model** | `target` (number, −60..0, default −1.0) |
| **Remove DC offset** before scaling | **in-model** | `remove_dc` (boolean, default false) |
| **Per-channel vs linked** stereo normalization | **in-model** | `per_channel` (boolean, default false = linked) |
| Output bit depth (16 / 24 / 32-bit float) | **in-model** | `bit_depth` (enum `16`\|`24`\|`32f`, default `16`) |
| Report measured peak / applied gain / new peak | **in-model** | in the result summary text |
| Refuse to normalize digital silence (peak = 0) | **in-model** | clear error |
| Accept common input formats (wav/flac/mp3/ogg/m4a) | **in-model** | symphonia decode; **always outputs WAV** |
| **True-peak (inter-sample / oversampled) targeting** in dBTP | **out-of-model** | the row says "loudest *sample*" = sample peak; true-peak/dBTP is `audio-normalize`'s domain (loudnorm caps TP) — listed, not built |
| **Loudness (LUFS/RMS) normalization** | **out-of-model** (that IS `audio-normalize`) | — |
| Real-time preview / waveform meters | **out-of-model** (no interactive page) | — |
| Batch of many files at once | **out-of-model** (one file per call) | — |
| Re-encode back to a lossy container (mp3/m4a out) | **out-of-model** (no ffmpeg encoder in a pure block) | WAV out only; note it |

### Copy / UX notes (paraphrased only — no competitor copy or branding reused)
- Default target **−1.0 dBFS** (Audacity/SoX convention: just under full scale, leaving headroom
  for downstream processing/encoders); expose the full −60..0 range so users can pick 0 (maximize)
  or a conservative −3/−6.
- `per_channel` defaults to **linked** (false) so stereo imaging/balance is preserved, matching the
  Audacity/Audition/SoX default; the on-path rebalances channels independently.
- `remove_dc` defaults **off** so the default operation is a pure, predictable peak scale; document
  that Audacity defaults it on and when to enable it (audible click/asymmetric waveform).
- Summary reports measured peak (dBFS), applied gain (dB), and resulting peak so the user can verify
  the ceiling was hit — the transparency SoX/ffmpeg-normalize print to stderr.

## Sources
- Audacity Manual — Normalize: https://manual.audacityteam.org/man/normalize.html
- Audacity Manual — DC offset: https://manual.audacityteam.org/man/dc_offset.html
- Wikipedia — Audio normalization (peak vs loudness, sample vs true peak): https://en.wikipedia.org/wiki/Audio_normalization
- ffmpeg-normalize (peak mode, target dBFS): https://pypi.org/project/ffmpeg-normalize/

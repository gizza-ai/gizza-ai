# document-skew-detector — competitor analysis (2026-07-20)

Function: estimate the skew (rotation) angle of a scanned document or text image so it can
be deskewed. Our tool is a **detector/analyzer** (reports the angle + suggested correction),
not a straightener — rotating to correct is the existing `rotate-image` tool's job. Sibling
of `image-horizon-tilt-checker` (photo horizons via edge orientation); this one keys on
TEXT LINES via the projection-profile method, so it works on pages with no ruled lines.

## Competitors scanned (paraphrased — no copy reproduced)

1. **sbrunner/deskew (Python library + CLI)** — the reference open-source deskewer: Hough
   transform on edges, returns the correction angle in degrees. Default result range −45..45
   (an `angle_pm_90` option extends to ±90 with orientation ambiguity); tunables `min_angle`/
   `max_angle` (search window), `num_peaks`, `sigma`, `min_deviation`. Its bundled test scans
   (with published expected angles) were used as our accuracy ground truth.
2. **ImageTools.org Deskew** — browser tool, batch upload; single control: a binarization
   **threshold percentage** (default ~40%, "works for most images"); fully automatic angle
   detection + rotation; download result.
3. **PDFGenies Deskew** — upload PDF/JPG/PNG → fully automatic detect + straighten +
   download; zero parameters, zero learning curve; privacy framing (no server storage).
4. **Aspose OCR Skew (CalculateSkew/AutoSkew)** — commercial API: `CalculateSkew` returns
   the tilt angle in degrees per page; `AutoSkew` applies it; manual `Rotate` as fallback;
   noted to struggle with strong perspective distortion (recommends manual handling).

Also referenced as method prior art (not scanned as products): ImageMagick `-deskew
threshold%` and Leptonica's sweep-search — both projection-profile family, which is what we
implement.

## Table-stakes params / defaults / patterns

| Capability | Competitor norm | Fit | Our decision |
|---|---|---|---|
| Auto skew detection from text lines | all four | in-model | core projection-profile sweep (coarse 0.5° → fine 0.05° → parabolic refine, 0.01° resolution) |
| Angle search range | ±45 (sbrunner default), APIs ~±15 | in-model | `max_angle` number, default 15, range 1–45 |
| Binarization threshold control | ImageTools slider (~40%), ImageMagick `-deskew N%` | in-model | `threshold` integer percent 0–99, default 0 = automatic (Otsu) |
| Correction angle + direction | all report/apply an angle | in-model | output `suggested_rotation_degrees` + `direction` (straight/clockwise/counterclockwise/undetermined) |
| Zero-config happy path | PDFGenies, Aspose AutoSkew | in-model | every param defaulted; `url=` alone works |
| "Already straight" snap | tools snap near 0 | in-model | `tolerance` number, default 0.5°, range 0–10 → `is_straight` |
| Detection confidence | implied by APIs | in-model | output `confidence` 0–1 (peak prominence) + `ink_pixels` + `threshold_used` |
| Dark scanner-bed background robustness | real-scan test sets include it | in-model | vertical-run filter keeps only text-like thin strokes (fixed a total miss on such a scan) |
| White-on-black (negative) scans | handled implicitly | in-model | auto polarity inversion when ink would be the majority |
| Apply the rotation / output deskewed image | all online tools | out-of-scope here | pairs with `rotate-image` using our reported angle (stated in the tool description + note) |
| ±90 orientation / upside-down detection | sbrunner `angle_pm_90`, Aspose AutoSkew | out-of-model | 90°/180° orientation is an OCR-adjacent problem; our range is capped at ±45 and stated |
| Multi-page PDF batch | PDFGenies, Aspose | out-of-model | single image input; PDF tools are separate blocks |
| Perspective (keystone) correction | Aspose flags it as a separate problem | covered elsewhere | `document-scan` does 4-point dewarp; cross-referenced in the description |

## Accuracy verification (sbrunner test scans, their published expected angles)

| scan | expected | ours | note |
|---|---|---|---|
| deskew-1 (4152×6172, 25.6 MP) | −1.0 | −1.39 | their values are 1°-quantized (180-sample Hough) |
| deskew-2 | −2.0 | −2.19 | |
| deskew-3 | −6.0 | −6.16 | |
| deskew-4 (black scanner bed) | +7.0 | +7.05 | raw-ink profile failed at 0.0 before the run filter |
| deskew-5 | +3.0 | +3.41 | |
| deskew-6 | −3.0 | −3.34 | |
| deskew-7 | +3.0 | +3.40 | |

Differential accuracy (rotate deskew-5 by exact ffmpeg deltas, measure the change):
+2° → +2.00 measured, −3° → −3.00, +0.7° → +0.70 — sub-0.01° differential agreement, so
the sub-degree disagreements above are consistent with the competitors' 1° quantization.

## Worked example (our output shape)

Input: a scanned letter whose text lines climb to the right.
Output JSON: `angle_degrees: -2.19`, `suggested_rotation_degrees: 2.19`,
`direction: "counterclockwise"`, `is_straight: false`, `confidence: 0.62`,
`threshold_used: 127`, `ink_pixels: 57058`. Feed `2.19` to `rotate-image` to deskew.

## Surfaces

Image input + text/JSON report → **chat + CLI only, no standalone page** (the no-page
file-input pattern, same as `image-horizon-tilt-checker` / `image-info`). The 64 MiB
sandbox is respected via a header-first decode budget (input + decoded raster ≤ 48 MB,
clear "re-export at lower resolution" error beyond it) and a streaming box-downscale —
a 25.6-MP 1-bit scan runs in-sandbox.

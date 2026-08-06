# image-skin-tone-analyzer — competitor analysis (2026-08-06)

Scan done before implementing. Three real competitor tools skimmed; all findings below are
paraphrased observations of *capabilities*, never their copy, branding or trademarks.

## Tools reviewed

| # | Tool | Shape |
|---|------|-------|
| 1 | SkinBalance (skinbalance.app) — plus its published skin-tone colour-correction guide | Uploads a portrait, compares skin against a calibrated reference database, emits retouching slider values |
| 2 | ToneMatch (tonematch.pro) | Selfie in, CIE LAB measurement of sampled face regions, undertone + colour-season classification with per-class probabilities |
| 3 | Skin Tone Detector (faceshape-detector.com/skin-tone-detector) | Photo/camera in, undertone + depth category out in ~3 s, with lighting caveats |

Two more were sampled for cross-checking the vocabulary only (media.io skin tone detector,
beautylove.ai skin tone detector) — same feature set as #3, nothing new.

## Table stakes observed → decision

| Capability | Seen in | In model? | Decision |
|---|---|---|---|
| Detect skin pixels and measure their average colour | 1, 2, 3 | **in** | Built: rule-based YCbCr chroma-band detector (+ an RGB rule at `strict`), mean taken in **linear light**, reported as hex/RGB |
| Report the average **hue** | 1, 2, 3 | **in** | Built: `hue_degrees` (HSV) as the headline, plus `lab_hue_degrees` (CIELAB a\*b\* hue angle) which is what the undertone/cast logic actually uses |
| CIE LAB measurement of skin | 2 | **in** | Built: `lab_l` / `lab_a` / `lab_b` / `lab_chroma`, D65, gamma-correct |
| Undertone class: warm / cool / neutral / **olive** | 2, 3 | **in** | Built: 4-way `undertone` from the a\*b\* hue angle, with the boundary degrees stated in the output and the docs |
| Depth / tone scale (fair → deep) | 3 | **in** | Built as **ITA°** (Individual Typology Angle, `arctan((L*−50)/b*)`) with the published Chardon bands → `depth` label. Objective and reproducible, unlike a proprietary scale |
| Warm/cool **white-balance correction suggestion in Kelvin** | 1 | **in** | Built: solved numerically on the Planckian locus (see below) → `suggested_kelvin` + `suggested_kelvin_shift` |
| Adjustable sensitivity of the skin mask | — (all are fixed) | **in** | Built: `sensitivity` = strict / normal / loose. A gap in all three competitors: none let you widen the mask when the cast itself pushes skin out of band |
| "Lighting isn't good enough to read" guard | 2, 3 | **in** | Built: `min_skin_percent` gate, a `clipped_percent` blown/crushed count, a `confidence` score and an explicit `warnings` list |
| Tunable reference for "correctly balanced skin" | — | **in** | Built: `reference_hue` (default 52°). Competitors hard-code their reference; exposing it makes studio calibration possible |
| "Already balanced" tolerance | 1 (implicit) | **in** | Built: `tolerance` degrees → `is_balanced` |

## Explicitly out of model (listed, not built)

- **Face detection / sampling forehead-cheeks-jawline separately** (2, 3) — needs an ML face
  detector; gizza blocks are pure Rust with no model. Mitigated by segmenting skin across the
  whole frame and reporting coverage + confidence so a low-face-area photo is visible as such.
- **Tint (green–magenta) slider value** (1) — a second correction axis needs a second reference
  constraint; with a single reference *hue* the solve is degenerate. Emitting a number here would
  be fabricated, so nothing is emitted. (The Kelvin axis alone is well-posed.)
- **Exposure compensation value** (1) — a "correct" skin L\* depends on subject depth, which needs
  their calibrated per-depth database. We report the measured `lab_l` and flag clipping instead.
- **Colour-season assignment, palettes, colours-to-avoid, celebrity matches, virtual draping**
  (2, 3) — styling/product recommendations, not deterministic colour measurement.
- **Applying the correction to the image** (1) — this block is an analyser; the output note points
  at the existing `auto-white-balance` and `image-hsl-adjust` blocks, matching how
  `image-horizon-tilt-checker` points at `rotate-image`.
- **Batch / multi-photo sync** (1) — a chat/CLI invocation pattern, not a distinct capability
  (established precedent: the `strip-image-metadata` and `image-resizer` skiplist entries).

## How the Kelvin suggestion is derived (worth recording — it is the one non-obvious part)

The image is assumed to have been rendered at **5500 K** (stated in the output as
`assumed_capture_kelvin`, since a JPEG/PNG carries no reliable WB tag). For a candidate camera
temperature `K`, the correction gain is the componentwise ratio of the linear-sRGB white points of
the two illuminants, `W(5500)/W(K)`, renormalised to preserve luminance. Candidate temperatures are
scanned across 2000–12000 K and the one whose corrected skin colour lands closest to
`reference_hue` wins. Raising `K` warms the render and lowering it cools it, so a too-yellow skin
reading yields a negative shift — the same direction convention a raw editor's temperature slider
uses. Planckian `xy` comes from the standard cubic approximation of the locus (valid 1667–25000 K),
then `xy → XYZ → linear sRGB`.

Consequence worth stating on every surface: **the suggestion is a starting point, not a calibrated
measurement** — an unknown capture temperature and the subject's real skin hue are confounded. That
caveat is in the descriptor text and in the response `note`.

## Known limitation this class of tool shares (recorded, mitigated)

A rule-based skin mask is defined in chroma space, so a *severe* cast can push genuine skin pixels
out of the band and shrink the sample — the analysis then reports low coverage and low confidence
rather than a wrong answer. `sensitivity = loose` widens the band for exactly that case, and the
error message on a zero-pixel result says so.

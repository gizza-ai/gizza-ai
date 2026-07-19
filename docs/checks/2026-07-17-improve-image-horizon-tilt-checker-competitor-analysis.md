# image-horizon-tilt-checker — competitor analysis (2026-07-17)

Function: detect the tilt angle of a photo's horizon (or dominant vertical lines) so it can
be leveled. Our tool is a **detector/analyzer** (reports the angle + suggested correction),
not a straightener — rotating to correct is the existing `rotate-image` tool's job.

## Competitors scanned (paraphrased — no copy reproduced)

1. **Image Straightener (imageonline.io)** — manual straighten with an angle range of
   **−45° to +45°**, an optional **alignment grid** overlay to line the horizon up against
   horizontal/vertical reference lines, live preview, free/no-account.
2. **Fotor Photo Straightener** — gradient/guide lines plus a **slider** to nudge the angle;
   part of a larger rotate & flip toolbar; manual, browser-based.
3. **Image Tool Hub — Auto-Straighten** — the closest to our function: it **detects dominant
   lines and estimates a rotation angle**, then rotates to align horizontal/vertical
   structures. Auto-detection is the headline feature.
4. **Imagen AI / Evoto / Bylo.ai** — AI auto-straighten: analyze horizons, architectural
   lines and vertical subjects, report an angle deviation and one-click correct. (The
   detection step is in-model; the AI relighting/upscaling around it is out-of-model.)

## Table-stakes params / defaults / patterns

| Capability | Competitor norm | Fit | Our decision |
|---|---|---|---|
| Angle search range | ±45° (imageonline), sliders default ~±15° | in-model | `max_angle` number, default 15, range 1–45 |
| Auto-detect dominant line | Image Tool Hub, AI tools | in-model | core Sobel-gradient orientation histogram |
| Reference axis (horizon vs vertical lines) | AI tools straighten both horizons & verticals | in-model | `reference` enum `horizon`\|`vertical`, default `horizon` |
| "Already level" tolerance | implicit (tools snap near 0) | in-model | `tolerance` number, default 1.0°, range 0–10 |
| Suggested correction / direction | all report an angle to rotate | in-model | output `suggested_rotation_degrees` + `direction` (clockwise/counterclockwise/level) |
| Confidence of detection | AI tools imply it | in-model | output `confidence` 0–1 + `edges_analyzed` |
| Alignment grid overlay | imageonline, Fotor | out-of-model (visual editor UI) | N/A — this is a report tool (chat + CLI), pairs with `rotate-image` |
| Live rotate preview / one-click apply | most | out-of-model here | correct via `rotate-image` using our reported angle |
| AI relighting / upscaling | Imagen/Evoto | out-of-model | not built |

## Worked example (our output shape)

Input: a landscape photo whose sea horizon dips to the right.
Output JSON: `angle_degrees: 3.4`, `suggested_rotation_degrees: -3.4`, `reference: "horizon"`,
`direction: "clockwise"`, `is_level: false`, `confidence: 0.71`, `edges_analyzed: 4120`.
Feed `-3.4` to `rotate-image` to level it.

## Surfaces

Image input + text/JSON report → **chat + CLI only, no standalone page** (the F3 no-page
file-input pattern, same as `image-info` / `image-metadata-viewer`). Alignment-grid and
live-preview UX are editor features outside this repo's tool model; stated here, not built.

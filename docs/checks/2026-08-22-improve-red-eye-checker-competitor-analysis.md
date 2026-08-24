# red-eye-checker competitor analysis (2026-08-22)

## Scope

`red-eye-checker` is a local, in-browser and CLI detector for red-eye candidates. It reports pixel coordinates, radius, area and confidence; it does not edit or retouch the source image.

## Competitor scan

| Competitor/tool shape | Table-stakes capabilities observed | Gizza fit decision |
| --- | --- | --- |
| Fotor-style online red-eye remover | Upload a portrait, automatically find red pupils, preview edits, then export a corrected image. Controls are intentionally simple; the main promise is one-click correction. | Automatic detection is in-model and implemented. Pixel editing/beauty-retouch preview/export is out-of-model for this tool because the backlog item asks for checking/reporting locations, not modifying photos. |
| LunaPic-style red-eye correction editor | Upload an image, select or click the affected eye area, adjust a correction strength, and save an edited result. UX centers on manual targeting and visual preview. | Region-size filtering maps to `min_radius`/`max_radius`; sensitivity maps to strength-like control. Manual click-to-fix and edited output are listed as out-of-model. |
| General browser image editors / PineTools-style utilities | File upload, private local-ish processing messaging, simple form controls, worked examples, and clear limits for file type/size. | Implemented: browser upload, no server upload, examples, FAQ, PNG/JPEG limits, and a JSON report. Preset chips cover common strict/typical/high-sensitivity use cases. |

## In-model table-stakes implemented

- Image input: PNG/JPEG upload on the page; URL/ref image source for CLI/chat through `Input::Image`.
- Automatic detection: red-dominant saturated pixels are grouped into connected components and shape-filtered for pupil-like blobs.
- Tuning controls:
  - `sensitivity = low|medium|high`, default `medium`.
  - `min_radius`, default `3`, to ignore speckle/noise.
  - `max_radius`, default `80`, to reject large red objects.
  - `max_regions`, default `20`, to cap noisy reports while preserving total candidate count.
- Output: deterministic JSON with image dimensions, candidate count, per-region center/radius/area/average-red/confidence, and warnings.
- UX controls: select labels, slider controls for numeric bounds, and preset chips for typical/strict/high-sensitivity/group-photo cases.

## Out-of-model / intentionally not built

- Pixel correction, healing, recoloring, before/after preview, and edited-image export.
- Face/eye landmark detection or ML-based portrait understanding.
- Manual click/brush UI for choosing eye locations.
- Batch photo processing.

## Verification targets

The shipped checks should prove:

- A synthetic red-eye PNG is detected at the expected center.
- Neutral/blue images report zero candidates.
- Invalid or too-large inputs fail with actionable errors.
- Browser page accepts an uploaded fixture, honors deep-linked parameters, and renders real JSON output.

# video-ken-burns — competitor analysis (2026-07-23)

Function: animate a single still image with a smooth pan-and-zoom (Ken Burns) effect into a
short video clip. All notes are paraphrased from public marketing/help pages — no copy,
branding, or trademarks reproduced.

## Competitors skimmed

1. **imagepanzoom.com** — single-image → pan/zoom video, in-browser, local export. Controls:
   draw a focus region (click-drag), reposition/resize the region, a "Zoom out at the end"
   toggle, Preview, Export Video (single) / Export ZIP (batch). Output: video (MP4). Very
   simple; no exposed duration/zoom-amount/fps sliders — motion is inferred from the drawn
   region.
2. **VEED.io — Ken Burns effect** — pick a photo on the canvas, Animation → Ken Burns, choose
   "pan or zoom in or out". Movement is the main control; duration follows the clip. Output is
   part of a video project (MP4).
3. **Kapwing — Ken Burns / "Moving Zoom"** — one-click slow zoom-in or zoom-out; "adjust the
   speed, start, and end" of the effect. Output: MP4 export.
4. **Animotica (blog, reference for the vocabulary)** — enumerates the standard Ken Burns
   movement set: Zoom In/Out (Center/Left/Right), Pan Left/Right/Up/Down, Rotate Left/Right,
   with adjustable Scale (zoom amount) and Horizontal/Vertical Offset (pan).

## Table-stakes parameters (with the in/out-of-model decision)

| Capability | Competitors | Decision | Where it lands |
| --- | --- | --- | --- |
| Movement direction (zoom in / zoom out / pan L/R/U/D) | all | **in-model** | `direction` enum |
| Clip duration (seconds) | Kapwing (speed/start/end), VEED | **in-model** | `duration` |
| Zoom amount / scale / intensity | Animotica (Scale), imagepanzoom (implied) | **in-model** | `zoom` |
| Output resolution (WxH) | all export MP4 at a resolution | **in-model** | `width` + `height` |
| Frame rate | implicit | **in-model** | `fps` |
| MP4 output | Kapwing, imagepanzoom | **in-model** | output is `video/mp4` (H.264) |
| Draw a custom focus region on a canvas | imagepanzoom, VEED | **out-of-model** | interactive canvas UI — no single-shot param form; the `direction` presets cover the common motions instead |
| Rotate Left/Right (rotating Ken Burns) | Animotica | **out-of-model** | `zoompan` has no smooth continuous rotation; a rotating crop needs a separate `rotate` pass and jitters — deferred |
| Ease-in/ease-out timing curve | Kapwing ("speed") | **out-of-model (for now)** | kept linear for deterministic, jitter-free motion; noted as a limit on the page |
| Batch / multi-image slideshow | imagepanzoom (Export ZIP) | **out-of-model** | multi-image is the already-skiplisted `ken-burns-slideshow-video` (single-upload page + no chat SW ffmpeg) |

## UX patterns to match

- Direction as a small labelled preset list (friendly `[input.labels]`).
- Zoom amount + duration + fps as **sliders** (bounded numeric ranges).
- One-click **preset chips** (`[[example]]`) for the common looks: "Slow zoom-in", "Zoom out
  reveal", "Pan right", vertical 9:16 story.
- Fill-the-frame (cover) framing like every competitor.

## Implementation approach (in-model)

Single ffmpeg `zoompan` pass on the still image: pre-scale to cover WxH at a 2× supersample to
suppress zoompan's integer-grid jitter, then `zoompan` with linear `on`-based z/x/y expressions
(exact, not the drifty `min(zoom+step,max)` accumulator), `d = round(duration*fps)` frames,
`s = WxH`, encoded `libx264 -pix_fmt yuv420p +faststart`. Deterministic exact output dimensions
and duration — asserted end-to-end in Playwright.

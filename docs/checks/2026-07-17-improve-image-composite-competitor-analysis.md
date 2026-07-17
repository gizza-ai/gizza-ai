# image-composite — competitor analysis (2026-07-17)

Tool: **image-composite** — overlay/blend one image (foreground) onto another (background)
with position, scale, opacity and a blend mode. Built as a chat + CLI multi-image tool
(`Input::None` + `source_list("images", 2)`, image output; **no standalone page** — the page
model is a single file upload, so two-image tools follow the `image-split-overlay` /
`image-collage` / `gif-from-images` pattern of chat + CLI only).

## Competitors scanned (paraphrased — no copy/branding reused)

1. **Pixlr — Image Overlay** (`pixlr.com/tools/image-overlay/`) — upload base + overlay, layer,
   adjust opacity and blend mode (Multiply, Overlay, Screen among others), reposition on a canvas.
2. **ImageOverlay.org** — blend options normal, multiply, screen, overlay, soft-light; fine-tune
   opacity, scale, rotation, and positioning with sliders; border option.
3. **ThinkForU — Image Overlay** — drag to position, live opacity + blend; supports Normal,
   Multiply, Screen, Overlay, Darken, Lighten, Color-Dodge, Color-Burn, Soft-Light, Hard-Light,
   Difference, Exclusion.
4. **Tiny-Online.Tools — Image Overlay** — position via presets (center, corners) OR custom
   coordinates; adjust scale, opacity, blend mode (normal, multiply, screen).
5. **PicOverlay** — opacity, rotation, scale, flip controls with live preview and sliders.

## Table-stakes → decision

| Capability | Competitors | Ours | Tag |
|---|---|---|---|
| Two image inputs (background + overlay) | all | `images` source_list of 2 (A=base, B=overlay) | **in-model** |
| Blend modes multiply/screen/overlay | all | `blend_mode` enum: normal, multiply, screen, overlay, darken, lighten, hard-light, soft-light, difference, exclusion, add | **in-model** |
| Opacity | all | `opacity` 0.0–1.0 | **in-model** |
| Position presets (center/corners) | Tiny, Pixlr | `position` enum: center + 8 edge/corner anchors | **in-model** |
| Custom pixel coordinates / nudge | Tiny | `offset_x` / `offset_y` pixels (may be negative) | **in-model** |
| Scale the overlay | ImageOverlay, PicOverlay, Tiny | `scale` percent of overlay's native size (1–1000) | **in-model** |
| Flip overlay | PicOverlay | `flip` enum: none/horizontal/vertical/both | **in-model** |
| Output format | most | `format` enum: png (keeps alpha) / jpeg | **in-model** |
| Color-dodge / color-burn blend modes | ThinkForU | not shipped (rarer, and dodge/burn divide-by-zero edge cases add risk); the 11 shipped modes cover every table-stake named in the tool description and by ≥3 competitors | considered, rejected (scope) |
| Arbitrary-angle rotation | ImageOverlay, PicOverlay | out-of-model: the pure-Rust `image` crate (default-features off) has only 90°-step rotation, not dep-free arbitrary-angle resampling | **out-of-model** |
| Live drag positioning / real-time preview / sliders | all | out-of-model: chat + CLI is deterministic-params, not an interactive canvas; presets + numeric offset are the deterministic equivalent | **out-of-model** |
| Border around overlay | ImageOverlay | considered, rejected (secondary; `image-border-frame` already frames a single image) | considered, rejected |

## Compositing model

Background = image A at its native size (pixel-capped). Overlay = image B scaled by `scale`%,
optionally flipped, placed at the `position` anchor plus (`offset_x`,`offset_y`) and clipped to the
canvas. Each overlapping pixel uses the W3C separable blend-then-source-over formula
(`Co = (αs·blend + αb·(1−αs)·Cb) / αo`, `αs = overlay_alpha × opacity`), so transparent overlay
regions and a transparent background both composite correctly, and `opacity` scales the overlay's
contribution. PNG preserves alpha; JPEG flattens transparency.

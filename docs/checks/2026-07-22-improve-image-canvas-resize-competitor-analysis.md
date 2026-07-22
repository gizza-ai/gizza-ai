# image-canvas-resize — competitor analysis (2026-07-22)

Tool: change an image's **canvas** dimensions to an exact width × height **without scaling the
pixels** — grow adds margin (fill color) around the content, shrink crops it, both positioned by a
9-point anchor. This is the ImageMagick `-extent` / Photoshop *Canvas Size* operation, distinct from
`image-resize`/`image-contain-fit`/`image-cover-fit` (all of which *scale* the content) and from
`image-crop` (fixed rectangle, no padding). Pure-Rust `image`; chat + CLI (image-bytes output → no
page, same surface family as `image-contain-fit` / `image-cover-fit`). All findings paraphrased — no
competitor copy, branding, or trademarks reproduced.

## Competitors surveyed (top 3)

1. **ImageMagick `-extent`** (usage.imagemagick.org/resize, discourse archive). The canonical CLI
   primitive: `-background <color> -gravity <pos> -extent WxH`. `-extent` alone (no `-resize`) keeps
   the content at native pixel size and only adjusts the canvas — pads with `-background` when the
   target is larger, crops when smaller. `-gravity` (center/north/south/east/west + corners) decides
   where the content sits and, on shrink, what is cropped away.

2. **Photoshop / Photopea "Canvas Size"** (community.adobe.com, glensmith.co.uk). Explicitly the
   counterpart to *Image Size*: changes only the document bounds, layers keep their scale. A 3×3
   **Anchor** grid decides where the added canvas goes (center = equal on all 4 sides) and, when
   reduced, which direction the crop aims. A background/canvas-extension color is selectable. Also
   offers a **Relative** mode (add N px/percent to current size) and per-unit input.

3. **Online canvas/pad tools** (Pixlane Expand Canvas, A.Tools Image Canvas Resizer, OnlineMiniTools
   Pad Image, Vayce Image Canvas Adjuster). Common feature set: exact target W×H (or aspect preset),
   a **3×3 anchor grid**, a **solid fill color OR keep transparent**, browser-side processing. Extras
   seen: per-side padding, eyedropper/sample-from-image fill, aspect-ratio presets.

## Table stakes → mapped to our params

| Capability | In model? | How we cover it |
|---|---|---|
| Exact target width × height (px) | ✅ | `width`, `height` integers (required, ≥1) |
| No scaling of source pixels | ✅ | core places source at native size; never resamples |
| Grow → pad with fill color | ✅ | canvas filled with `fill`, source blitted per anchor |
| Shrink → crop by anchor | ✅ | same placement math, out-of-canvas source pixels dropped |
| 9-position anchor / gravity | ✅ | `anchor` enum: center + 4 edges + 4 corners |
| Fill/background color (hex + named) | ✅ | `fill` accepts `#rgb`/`#rrggbb`/`#rgba`/`#rrggbbaa`, names |
| Keep transparent (alpha fill) | ✅ | `transparent`/`none` → RGBA(0,0,0,0); PNG output keeps alpha |

## Out-of-model / deferred (listed, not built)

- **Per-side padding** (independent top/right/bottom/left) — OnlineMiniTools/Photoshop offer it. Our
  scope is absolute canvas W×H + anchor; per-side amounts are a different input model. Deferred.
- **Relative mode** (add N px / N% to current size) — Photoshop's Relative checkbox. Absolute W×H is
  the primary, unambiguous form; relative is a convenience layer over it. Deferred.
- **Aspect-ratio presets / chips** (1:1, 16:9) — a page-UI affordance; this tool has no page
  (image-bytes output), so presets don't apply to the chat/CLI surface.
- **Eyedropper / sample fill color from the image** — an interactive-canvas feature; not expressible
  as a headless parameter.
- **Output format choice (jpg/webp)** — we always emit **PNG** so the transparent/padded areas
  survive (JPEG has no alpha), matching `image-contain-fit`/`image-cover-fit`. Format conversion is
  covered by the dedicated `image-convert` tool. Deferred.
- **Generative / blurred background fill** — needs an ML model; out of the pure-Rust model.

## Decision

Build pure-Rust `image` with `width`, `height`, `anchor` (enum, 9 gravities), `fill` (color string),
always-PNG output. Every table-stake capability is either a parameter or the core behavior; every
deferred item is listed above, none dropped silently.

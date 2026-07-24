# image-perspective-warp — competitor analysis (2026-07-22)

Tool function: apply a free four-corner perspective transform to an image (keystone / skew
correction, or deliberate perspective distortion). Built on ffmpeg's `perspective` filter. A spike
found that the filter reliably accepts pixel numbers plus the `W`/`H` edge constants, but browser
ffmpeg builds do **not** consistently evaluate arithmetic such as `0.12*W` for these coordinates,
so the shipped model exposes direct pixel coordinates instead of normalized percentages.

## Competitors skimmed (paraphrased — no copy/branding reproduced)

1. **imageonline.io – Perspective Tool** — four draggable corner handles, real-time preview,
   "Reset Points". Output PNG / JPG / WebP. Optional white background for PNG. No numeric
   coordinate entry, no interpolation setting, no dimension control, no presets.
2. **imagy.app – Perspective Image** — four draggable corners; a **Correct** mode (straighten a
   skewed object) vs **Distort** mode (add a tilt); customizable **background fill** for exposed
   areas; broad input/output format list; batch "Apply to all" → ZIP; all processing local.
3. **imageonline.io – Perspective Crop** — four draggable corners to match a document's corners,
   live preview, Reset, paste-to-upload, copy-to-clipboard, download. Output PNG / JPG / WebP,
   "original size" vs smaller. No interpolation/quality knobs exposed.

## Table-stakes → decision (each tagged in-model / out-of-model)

| Capability | Competitors | Our decision |
|---|---|---|
| Four-corner control (TL, TR, BL, BR) | all 3 | **in-model** — 8 coordinate params (`tl_x`…`br_y`) accepting pixel numbers plus `W`/`H` edge constants |
| Correct vs Distort mode | imagy | **in-model** — `mode` enum → ffmpeg `sense=source` (correct) / `sense=destination` (distort) |
| Interpolation / quality | ffmpeg offers; none expose | **in-model differentiator** — `interpolation` enum `linear`/`cubic` |
| Reset / copy result / paste-to-upload / download | all 3 | **in-model, free from platform** — generator supplies Reset, Copy, paste-upload, download |
| Preset examples (keystone, deskew, tilt) | — (competitors drag instead) | **in-model** — `[[example]]` chips stand in for presets |
| Real-time drag-handle preview on a canvas | all 3 | **out-of-model** — the declarative page has no interactive corner-drag canvas; we ship coordinate inputs + preset chips instead. A custom canvas widget with pointer math is deferred, listed not built. |
| Custom background fill for exposed areas | imagy | **out-of-model** — ffmpeg's `perspective` filter exposes no fill-color option; recoloring only exposed pixels needs fragile keying that can't distinguish warped-black from genuine black image content. Listed, not built. |
| Output-format choice (PNG/JPG/WebP) | all 3 | **considered, rejected** — kept output format = input to stay focused; format conversion is already covered by the `image-convert` tool. |
| Batch "Apply to all" → ZIP | imagy | **out-of-model** — single-file page/CLI model; no multi-file batch. |

## Worked defaults

- Corners default to identity (`tl 0,0`, `tr W,0`, `bl 0,H`, `br W,H`) — matches ffmpeg's
  own no-op default; example chips demonstrate real keystone/deskew/tilt warps using pixels.
- `mode = correct` (source sense — the given corners are where a rectangle currently sits in the
  photo; they are stretched to the full output rectangle).
- `interpolation = linear`.

## ffmpeg mapping (feasibility spike done)

`perspective=x0=<tl_x>:y0=<tl_y>:x1=<tr_x>:y1=<tr_y>:x2=<bl_x>:y2=<bl_y>:x3=<br_x>:y3=<br_y>:interpolation=<linear|cubic>:sense=<source|destination>`

Corner index order in the filter is TL(0), TR(1), BL(2), BR(3) — verified against the filter docs.
`W`/`H` work for identity defaults; arithmetic forms like `12/100*W`, `(12/100)*W`, and `0.12*W`
were tested against native ffmpeg and the browser page and behaved like no-ops for this filter, so
they are rejected rather than advertised.

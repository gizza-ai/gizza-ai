# signature-extract — competitor analysis (2026-07-13)

Function: take a photo/scan of a handwritten signature on paper and produce a clean
transparent PNG (just the ink, paper knocked out) for dropping into PDFs / e-sign forms.

## Competitors skimmed (paraphrased — no copy/branding reproduced)

1. **Online PNG Tools — "Extract a Signature from an Image"** (onlinepngtools.com) — the
   canonical algorithmic (non-AI) tool. Controls: pick/click the ink color; **color
   tolerance** (default ~20%, widenable to 30-40%) = how much color variance counts as
   ink; **recolor** the extracted ink to a new shade (blank = keep original); optional
   **opaque background** color (blank = transparent); **remove excess space** (crop white
   margin); **smooth signature edges** with an adjustable smoothing depth; optional
   **outline** (color + width). All client-side JS.
2. **Small PNG Tools — signature extractor** (smallpngtools.com) — simpler: **background
   color** picker (default white paper), **threshold** (default ~30%), **smooth** edges,
   plus recolor (black/blue). Drag-drop, live transparent-canvas preview, undo/redo. Free
   tier capped at 10 extractions/tool/day.
3. **AI background removers** (Pixelcut, Pokecut, PicFixer, AnyEraser, Picsart) — one-click
   "remove background from signature" via a segmentation model, download transparent PNG.
   No thresholds exposed; the model does the ink/paper separation.

## Table-stakes → decision

| Capability | In/out-of-model | Decision |
|---|---|---|
| Isolate ink, knock out paper → transparent PNG | in-model (luminance threshold) | **core behavior** |
| Threshold / ink sensitivity | in-model | **`threshold` 0-100 (default 50)** — higher keeps fainter strokes |
| Recolor extracted ink (black/blue/red or keep original) | in-model | **`ink` enum** original\|black\|blue\|red (default original) |
| Remove excess space / auto-crop to signature | in-model (alpha bounding box) | **`trim` bool (default true)** |
| Smooth / anti-aliased edges | in-model (soft alpha ramp) | **`smooth` bool (default true)** — soft alpha vs hard 1-bit |
| Opaque background color instead of transparent | in-model but off-purpose | **listed, not built** — the product IS a transparent PNG; an opaque fill defeats e-signing and is a one-liner elsewhere (image-bg-replace). Keep the param set tight. |
| Color-tolerance / click-a-color ink hue | in-model but UX-bound | **listed, not built** — needs an interactive eyedropper; luminance threshold covers dark-ink-on-light-paper, the dominant case. |
| Signature outline (color + width) | in-model but niche | **listed, not built** — rarely wanted for e-sign; adds two params for a decorative edge. |
| AI/ML segmentation for busy/colored backgrounds | out-of-model | **listed, not built** — gizza is pure-Rust + ffmpeg, no model. Threshold approach targets ink-on-paper. |

## Shape
Pure-Rust `image` crate (decode → luminance → soft alpha knockout → optional recolor →
optional trim → RGBA PNG). Runs on all backends incl. chat SW. Surfaces: **chat + CLI, no
standalone page** (image-in / image-bytes-out — page file-input path is ffmpeg-only, same as
image-document-shadow-remove / image-to-sketch).

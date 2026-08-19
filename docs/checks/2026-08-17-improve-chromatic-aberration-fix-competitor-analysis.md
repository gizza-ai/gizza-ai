# chromatic-aberration-fix — competitor analysis (2026-08-17)

Scan run BEFORE implementing. One web search ("online tool remove chromatic aberration purple
fringing photo defringe"), then three reachable competitor tools skimmed. All findings below are
**paraphrased** — no competitor copy, branding, or trademark text is reproduced or reused.

Adobe's own Camera Raw lens-correction help page (`helpx.adobe.com/camera-raw/using/correct-lens-distortions-camera-raw.html`)
timed out twice and was **replaced** with a third-party walkthrough of the same Lightroom panel.

## Competitors skimmed

### 1. Unpurple / `purple-fringe` (open-source CLI, github.com/mjambon/purple-fringe)
- **Function:** removes axial (longitudinal) CA — the purple/violet halo — from JPEG photos.
- **Algorithm (as documented):** build a blurred mask from the blue channel, subtract a
  mask-proportional amount of blue and red, then enforce three constraints: blue may not fall
  below green, red may not fall below green, and the red:blue ratio has a floor.
- **Params/defaults:** the README does not publish slider names, ranges, or defaults — it is a
  one-shot "run it on the file" binary. No tunables surfaced.
- **I/O:** JPEG in → JPEG out, quality fixed at 75, EXIF dropped. ~1 s per megapixel.
- **Stated limits:** purple only (cannot remove green fringing); corrected areas can end up
  greyer than they should; axial CA only, not lateral.

### 2. Exposure X (Exposure Software) — Chromatic Aberration + Defringe panels
- **Two separate panels**, which is the important structural signal: a *lens/lateral* CA panel
  and a *defringe* panel for whatever colour is left over.
- **CA panel controls:** a Blue slider (purple ↔ lime-green fringes) and a Red slider (red ↔ cyan
  fringes) — i.e. per-channel lateral correction; plus two corner sliders that scale the strength
  per corner. Guidance says the first two sliders cover most cases.
- **Defringe panel controls:** a global "remove fringes on all edges at once" action, plus per-colour
  hue + range controls to target one specific leftover fringe colour.
- **UX:** lens profiles, viewing at 100 % zoom while adjusting, saveable presets on both panels.
- No numeric ranges or defaults published on the page.

### 3. Magic Eraser (magiceraser.live) — online chromatic-aberration removal
- **Function:** neutralise purple / green / magenta halos on high-contrast edges while keeping the
  edge sharp; explicitly generative-AI based rather than per-channel lens correction.
- **Controls:** none numeric — the whole interaction is upload → brush over the fringed edges →
  apply → inspect at full resolution. A *brush/local-mask* UX rather than sliders.
- **I/O:** web/iOS/Android upload; input formats, size caps and output format are not stated.
  Advises feeding the full-resolution file, not a downscaled export.
- **Free tier covers this operation.**
- **FAQ topics they answer:** how this differs from glare/grain removal; whether edges go soft or
  grey after correction; behaviour on blown highlights; what the free tier includes.

### 4. Lightroom / Camera Raw Defringe (reference point, via a third-party walkthrough)
- **Workflow:** an automatic "remove chromatic aberration" checkbox first (lateral CA), then manual
  Defringe for stubborn colour: Amount sliders plus Purple Hue and Green Hue *range* sliders.
- **Eyedropper:** click the fringe in the image to set the hue range automatically; shows a
  magnified loupe at the cursor for accurate sampling.
- The walkthrough does not publish numeric ranges/defaults and does not warn about desaturating
  legitimately purple or green subject matter — a real gap we can close in our own copy.

## Table stakes → in-model / out-of-model

| Table stake | Where it lands |
|---|---|
| Separate purple and green fringe strength | **in-model** → `purple_amount`, `green_amount` (0–20, defaults 8 / 5) |
| Restrict the fix to high-contrast edges (the whole point) | **in-model** → `edge_threshold` (0–255, default 20) + a separable sliding-max edge dilation |
| Reach of the fix around an edge | **in-model** → `radius` (1–20 px, default 4) |
| Targeting a specific fringe hue / hue range | **in-model** → `hue_tolerance` (5–90°, default 40) around fixed purple (285°) and green (120°) centres |
| Lateral (red/cyan, blue/yellow) CA correction as its own control | **in-model** → `red_shift`, `blue_shift` (−10…10 px measured at the frame corner, default 0), radial per-channel bilinear resample applied BEFORE defringe — same split Exposure X uses |
| "Blue/red may not drop below green" constraint | **in-model** → the correction only removes the channel's *excess over green* (purple) or green's excess over max(R,B) (green fringe), so a channel can never cross green |
| Preserve alpha / not forcing JPEG-75 like Unpurple | **in-model** → `format` (`auto`/`png`/`jpeg`/`webp`, default `auto` = keep the input's format, PNG keeps alpha) + `quality` (1–100, default 90) |
| Green fringing at all (Unpurple can't) | **in-model** → shipped |
| Honest limit statement about purple/green subjects | **in-model** → stated in the descriptor copy + module docs (no page exists for this tool, see below) |
| Eyedropper / brush-the-fringe local mask | **out-of-model** — needs an interactive canvas surface; this block has no page (binary image in *and* out ⇒ chat + CLI only) |
| Lens-profile / EXIF-driven auto correction | **out-of-model** — needs an LCP/lensfun lens database plus EXIF lens matching, i.e. a bundled dataset this repo does not carry |
| Per-corner strength scaling | **considered, rejected** — 4 extra params for a case that only appears on decentred/tilted lenses; the radial model covers the common symmetric case |
| Generative edge reconstruction (Magic Eraser) | **out-of-model** — needs an ML model; gizza is pure-Rust + ffmpeg |
| Presets / saved settings | **out-of-model here** — no page surface to hang preset chips on |

## Worked example carried into the tool copy

A 1-pixel-wide violet halo on a black→white edge (RGB ≈ `(150, 60, 190)`) sits ~90 above green in
red and ~130 above green in blue, with hue ≈ 285°. At `purple_amount=8` the excess is scaled down
by 8/20 = 0.4 of the way toward green, leaving ≈ `(114, 60, 138)`; at `purple_amount=20` it lands
exactly on neutral `(60, 60, 60)` — the "corrected edges go grey" behaviour Unpurple documents and
Magic Eraser's FAQ is asked about. That trade-off is stated in the descriptor rather than left for
the user to discover.

## Surface note (honest)

This is a **pure-Rust image-bytes-in / image-bytes-out** block, so — exactly like the shipped
`alpha-defringe`, `image-opacity` and `normalize-image` blocks — it has **no page**: the page
generator only supports a binary file input for `runtime = "ffmpeg"` / `"model"` tools, and
image-bytes output has no page render mode. Surfaces verified here are **CLI** and the
**descriptor/schema** the chat surface consumes. There is no Playwright spec because there is no
page to drive; that is a surface limitation, not a skipped check.

ffmpeg was considered as an engine to gain a page and rejected on capability: there is no defringe
filter in libavfilter, `rgbashift` only does a uniform whole-frame per-channel translation (not the
radial lateral model, and nothing hue- or edge-aware), `chromaber_vulkan` is Vulkan-only and absent
from the browser core build, and a `geq` expression could not express the edge-gated hue-windowed
correction that is this tool's entire function. Pure Rust is the accurate engine here.

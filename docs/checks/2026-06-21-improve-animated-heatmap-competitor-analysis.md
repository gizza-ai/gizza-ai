# animated-heatmap — competitor analysis (2026-06-21)

## What the tool does
Takes a time-ordered sequence of numeric matrices (each matrix = one frame, frames
separated by a blank line) and renders an **animated heatmap GIF** showing how the
values evolve. Pure-Rust (`image` crate GIF encoder, no ffmpeg) → runs on **all**
backends including the chat Service Worker. Surfaces: **chat + CLI** (no standalone
page — image-bytes output has no page render mode, same as `gif-from-images` /
`heatmap-chart`).

Params: `data` (required), `delay_ms` (10–60000, default 400), `cell_px` (4–200,
default 24), `scale_min` / `scale_max` (optional fixed color-scale bounds).

## Competitors surveyed
1. **Python seaborn + matplotlib `FuncAnimation`** (GeeksforGeeks / freeCodeCamp /
   tutorialspoint guides) — the canonical code recipe: a list of matrices →
   `sns.heatmap` per frame → save GIF (pillow) or MP4 (ffmpeg).
2. **`heatmapanimation` (PyPI)** — dedicated package, GIF/MP4 output, geojson-aware.
3. **R + gganimate / magick** (FlowingData "Animated GIF Heatmaps in R") — frame
   loop rendered to a GIF.
4. **heatmap.js `AnimationPlayer`** (patrick-wied) — JS time-series heatmap player,
   `{min, max, data}` frames (point-density heatmap, not a matrix grid).
5. **Flourish / Heatmapper2** — no-code web heatmap builders; Heatmapper2 has a
   time-point slider/scrubber; Flourish has animated transitions.

(No copy/branding/trademarks were reproduced from any competitor.)

## Gap analysis (fit-to-model)

| Capability | Competitors | gizza animated-heatmap | Verdict |
|---|---|---|---|
| Sequence of matrices → animated GIF | seaborn, heatmapanimation, R | **Yes** | at parity |
| **Consistent color scale across frames** | a recurring *correctness pitfall* in the seaborn guides (per-frame `vmin/vmax` makes frames non-comparable); good recipes pin `vmin/vmax` | **Yes — single global min/max over all frames by default** (the key design choice); equal values keep the same color | **ahead of the naive recipe** |
| Fixed/explicit scale bounds | seaborn `vmin/vmax` | **Yes** (`scale_min` / `scale_max`, validated min<max) | at parity |
| Per-frame delay / playback speed | all | **Yes** (`delay_ms`) | at parity |
| Cell/output size control | seaborn figsize, heatmapanimation | **Yes** (`cell_px`) | at parity |
| Sequential colormap (low→high) | seaborn cmaps | **Yes** (RdYlBu-reversed blue→yellow→red, shared with `heatmap-chart`) | at parity |
| Runs with **zero install, in-browser/CLI** | all competitors need Python/R/JS + a render backend | **Yes** (pure-Rust, no ffmpeg, runs even in the chat SW) | **ahead** |
| Loops forever | most GIF exporters | **Yes** (`Repeat::Infinite`) | at parity |

### Deliberately out of scope (in-model but not worth the cost / no surface)
- **Per-frame caption / timestep label / colorbar legend**: would need text
  rasterization into RGBA frames (no font dep in the pure pipeline; the SVG-based
  `heatmap-chart` does legends/labels but SVG can't be a *frame* in an animated
  GIF). The static `heatmap-chart` already covers labelled single-grid output.
- **MP4 output**: competitors offer it, but MP4 encode = ffmpeg, which can't run in
  the chat SW; GIF keeps the tool universal. `gif-to-mp4` exists if a user needs MP4
  from the produced GIF.
- **Colormap choice (viridis/etc.)**: single sequential map keeps it consistent with
  the sibling heatmap tools; a future `colormap` enum param is the natural extension.
- **Interactive scrubber / no-code UI** (Flourish, Heatmapper2): out of the
  chat/CLI/image-bytes model.

## Result
Core capability is at or ahead of competitors for the in-model surface. The
distinguishing strength is the **global color scale across frames** (the thing the
seaborn tutorials repeatedly get wrong) plus **zero-install pure-Rust** execution.
Verified: 8 unit tests (incl. global-scale-consistency + drift-guard schema test),
`wafer build` instantiates the chat block (411.8 KiB), CLI produces a valid
multi-frame `GIF89a` (60×40, 5 image descriptors for a 3-frame 3×2 input) and
surfaces parse/shape errors. No page surface (image-bytes output).

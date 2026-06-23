# function-grapher — competitor analysis (2026-06-23)

Tool: `blocks/function-grapher` — plots one or more `y = f(x)` expressions over an
x-range and returns a standalone SVG (axes, gridlines, value labels, a colored
curve per function, and a legend). Pure-Rust (core depends only on `meval`), so it
runs on every backend including the chat Service Worker. Surfaces: chat + CLI. No
standalone page (a pure-Rust image-bytes output has no page render mode — same as
`line-series-chart`, `scatter-chart`, etc.).

## Competitors surveyed (function-graphing tools)

All notes are **paraphrased** observations of capabilities/UX — no copy, branding,
or assets were reproduced.

1. **Desmos Graphing Calculator** — the category leader. Interactive canvas with
   live re-plot, unlimited expressions, sliders/animation, points & data tables,
   inequalities, polar/parametric, regressions, table-driven plots. Heavy WebGL UI.
2. **GeoGebra Graphing Calculator** — interactive plot + algebra view, sliders,
   construction tools, derivatives/intersections, polar/parametric, export.
3. **Symbolab / Mathway graphing calculators** — expression entry with a graph plus
   *symbolic analysis* (intercepts, asymptotes, extrema, derivatives, step output).
4. **miniwebtool Function Grapher** — closest direct analog: three function slots
   (f/g/h), explicit x **and y** min/max window controls, broad function set
   (trig + inverse + hyperbolic, exp, ln, abs, roots), and it annotates intercepts,
   asymptotes, derivatives, and critical points with LaTeX-rendered formulas.
5. **analyzemath / aimathcalculator graphing calculators** — single/few function
   plots, settable window, intercept/root readouts, basic styling.

## Capability diff vs our tool

| Capability | Competitors | gizza function-grapher | Status |
|---|---|---|---|
| Plot `y=f(x)` over a range | yes | yes | parity |
| Multiple functions, distinct colors + legend | yes (3+) | yes (unbounded, 8-color palette, per-curve legend) | parity |
| Arithmetic `+ - * / ^`, parentheses | yes | yes (via `meval`) | parity |
| Trig / sqrt / abs / ln / log10 / exp, `pi`/`e` | yes | yes (`meval` builtins) | parity |
| Custom x-window (xmin/xmax) | yes | yes | parity |
| **Custom y-window (ymin/ymax)** | yes (miniwebtool, analyzemath) | **added this pass** — set both to crop; omit to autoscale; curve clipped via SVG `clipPath` | **closed** |
| Axis labels + gridlines | yes | yes (5×5 ticks, compact number formatting) | parity |
| Emphasized x=0 / y=0 axes | yes | yes (drawn only when 0 is in range) | parity |
| Discontinuity handling (e.g. `1/x`) | varies | yes — non-finite samples break the polyline into separate runs | parity / edge |
| Named curves in legend | partial (f/g/h labels) | yes — optional `name:`/`name=` prefix per expression (`y=` treated as unlabeled) | parity+ |
| Adjustable resolution | implicit | yes — `samples` (2..4000) | parity |
| Title + canvas size | some | yes (`title`, `width`, `height`) | parity |

## Gaps closed this pass (in-model)

- **Optional fixed y-window (`ymin`/`ymax`).** Previously the y-axis always
  autoscaled to the data; competitors let you pin the view. Added both as optional
  number params (omit → autoscale, which stays the default). When both are set the
  view is cropped and curves are clipped to the plot rect via an SVG `clipPath`, so
  unbounded functions (e.g. `tan(x)`, `1/x`) render cleanly inside the window.
  Backed by 3 new unit tests (manual-window labels+clip, single-bound falls back to
  autoscale, inverted-window rejected) + the regenerated drift-guard schema.

## Considered, not built (out-of-model or out-of-scope)

- **Interactive pan/zoom/sliders/animation** (Desmos/GeoGebra) — needs a live JS
  canvas UI; our output is a static SVG. Not in the chat/CLI image-bytes model.
- **Symbolic analysis** (intercepts, asymptotes, extrema, derivatives, step-by-step)
  — a different tool class (a CAS), out of scope for a plotter; could be a separate
  future tool. `meval` evaluates numerically and offers no symbolic differentiation.
- **Polar / parametric / inequalities / implicit curves / data-table plots** — each
  is a distinct input model; `line-series-chart`/`scatter-chart` already cover raw
  data-point plotting, so this tool stays focused on `y=f(x)`.
- **LaTeX-rendered formula annotations** — `latex-math-to-svg` already exists for
  rendering math; not duplicated here.
- **Accounts / cloud save / export-to-PNG-server** — gizza is browser-local,
  no-account, no-server by design; the SVG is directly downloadable/CLI-writable.

## Verification (this pass)

- `cargo test --workspace`: 17 core + 1 block drift-guard test — all green.
- `wafer build`: chat `block.wasm` builds and **instantiates** (440 KiB).
- CLI: `gizza tool function-grapher functions='sin(x); parabola: x^2' …` → 2-curve
  SVG with correct legend; `tan(x)` with `ymin=-5 ymax=5` → clipped window with
  `clipPath` + manual `5`/`-5` y-labels; error paths (`z^2`, inverted range) report
  helpful messages.
- Page: N/A — image-bytes output has no page render mode (stated, not skipped).

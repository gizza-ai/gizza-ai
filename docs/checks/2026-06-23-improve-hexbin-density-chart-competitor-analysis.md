# hexbin-density-chart — competitor analysis (2026-06-23)

Tool: `blocks/hexbin-density-chart` — bins x/y point data into a flat-top hexagonal
grid and renders the per-hex point count as a density heatmap (Viridis-like sequential
colour ramp), output as **SVG** or **PNG**. Pure-Rust (hand-built SVG + resvg/tiny-skia
PNG rasterization), so it runs on chat + CLI. No standalone page (image-bytes output has
no page render mode, same as `scatter-chart` / `line-series-chart`).

## Surfaces verified
- **chat** — `build_media_envelope` (image/svg+xml | image/png data-URL). Validated by
  `wafer build` (the resvg/usvg/tiny-skia chain instantiates in wasm32-wasip1).
- **CLI** — `gizza tool hexbin-density-chart data='[[...],...]' [title= width= height= radius= format=svg|png]`
  produced a 3.6 KB SVG and a valid 19–20 KB PNG (PNG magic bytes verified); the bad-JSON
  path returns a clean `data` error message.
- **page** — none (image-bytes output; not applicable).

## Competitors surveyed
1. **Plotivy — Hexbin Plot** (plotivy.app) — "publication-ready hexbin plot with AI", no
   coding; upload data → styled hexbin.
2. **ChartLoad scatter generator** (chartload.com) — free online scatter generator that
   recommends hexbin/density heatmaps once a scatter exceeds ~500 points.
3. **ggplot2 `geom_hex`** (tidyverse) — the reference R implementation: hexagonal heatmap
   of 2-D bin counts, fill = count, configurable `bins`/`binwidth`.
4. **Matplotlib `plt.hexbin`** — `gridsize`, `C`/`reduce_C_function` (aggregate a third
   value per cell), `bins='log'`, choice of colourmap.
5. **d3-hexbin** (web library) — hexagonal binning layout primitive; `radius()` controls
   hex size, colour mapped by count; powers most bespoke web hexbin charts.

## Feature diff + gaps (fit-to-model)

| Capability | Competitors | This tool | Status |
|---|---|---|---|
| Hex grid binning of x/y points | all | yes (flat-top lattice, nearest-centre assignment) | in parity |
| Count → colour density | all | yes (Viridis-like perceptual ramp) | in parity |
| Configurable bin size | `gridsize`/`binwidth`/`radius()` | yes (`radius` px, 4–120) | in parity |
| SVG + PNG export | d3 (SVG), mpl (PNG/SVG/PDF), Plotivy (PNG/SVG) | yes (svg + png) | in parity |
| Axes with min/mid/max ticks | most | yes | in parity |
| Colour legend (count scale) | most | yes (vertical gradient, 1→max) | in parity |
| Title | all | yes | in parity |
| Aggregate a 3rd value per hex (`C`/`reduce_C_function`) | matplotlib | no | **out of scope** (input model is x/y only; adding a weight column is a future improvement, not a blocker) |
| Log colour scaling (`bins='log'`) | matplotlib | no (linear) | minor gap; linear is the default everywhere else and adequate for the core use case |
| Colourmap choice | matplotlib/d3 | no (fixed Viridis-like) | minor; Viridis is the modern default and covers the primary need |
| Interactive tooltips / zoom | d3, Plotivy | no | **out of model** (gizza renders static images; no interactive surface) |
| Upload CSV file | Plotivy, ChartLoad | no (JSON array param) | **out of model** for this tool shape (JSON param matches scatter-chart; CSV→points is a separate parse step) |

## Decisions
- Shipped at parity with the core hexbin feature set (binning, density colour, configurable
  bin size, axes, legend, title, SVG **and** PNG). PNG is a real differentiator vs. the
  SVG-only existing gizza charts and matches Plotivy/matplotlib output expectations.
- Used a perceptual Viridis-like ramp (the modern default for density) rather than a rainbow
  map — better perceptual ordering, no extra dependency.
- **Not built** (honest gaps): third-value aggregation per hex (`C`/weight column), log colour
  scaling, colourmap selection, interactivity, and CSV upload — these are either out of gizza's
  static-image / single-JSON-param model or future enhancements that don't block the core tool.
- No competitor copy, branding, or trademarks were used.

## Sources
- [Plotivy — Hexbin Plot](https://plotivy.app/charts/hexbin-plot)
- [ChartLoad — Free Online Scatter Plot Generator](https://chartload.com/charts/scatter-plot/)
- [ggplot2 — geom_hex](https://ggplot2.tidyverse.org/reference/geom_hex.html)
- [R Graph Gallery — Hexbin with the hexbin package](https://r-graph-gallery.com/100-high-density-scatterplot-with-binning.html)
- [Think Design — Hexbin Visualization](https://think.design/services/data-visualization-data-design/hexbin/)

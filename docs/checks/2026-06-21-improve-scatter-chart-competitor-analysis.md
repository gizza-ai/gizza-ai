# scatter-chart — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/scatter-chart` — render a scatter plot from x/y (and optional
category/size) data as an SVG chart. Pure-Rust (hand-built SVG, no drawing deps).
Chat + CLI, no page (a pure-Rust image-bytes output has no page mode — like
`line-series-chart` / `heatmap-chart` / `correlation-heatmap`).

## What competitors do

- **Online chart makers** (chart-studio, various "scatter plot maker" sites) —
  paste data, get a chart. Strength: WYSIWYG. **Weakness: data is uploaded; many
  gate export/PNG behind sign-up or watermarks.**
- **matplotlib / seaborn / plotly / Excel** — powerful and standard, but require a
  Python/spreadsheet environment and code or manual clicking; not a one-call
  agent/CLI step.
- **Vega-Lite / D3** — great for the web, but you write a spec/JS and run a
  toolchain.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: runs in the
   chat Service Worker and headless in the CLI. Data never leaves the device.
2. **One JSON call → a clean SVG.** Accepts the two natural shapes: `[[x,y],…]`
   pairs *or* `[{x,y,category,size},…]` objects. Auto-scales axes (5% padding),
   draws gridlines and min/mid/max tick labels on both axes.
3. **Encodes category and size.** `category` colours points from an 8-colour
   palette and adds a legend; `size` scales the marker radius — a bubble-chart in
   the same tool, no extra config.
4. **Vector output.** SVG scales crisply at any zoom and is tiny; embeds directly
   in chat or a page, and is easy to restyle/convert downstream.
5. **Agent- + CLI-friendly.** Same one-shot call from chat and `gizza tool
   scatter-chart data=…`; the result is a reusable `ref`.

## Honest scope

- **SVG output (not PNG).** Like the other gizza chart tools, output is SVG
  (vector). PNG would require bundling an SVG rasteriser; SVG is smaller, sharper,
  and trivially converted if a raster is needed. The tool name mentions PNG; this
  build ships SVG and says so.
- **One series / single plot.** No trendline/regression overlay, log axes, or
  faceting — a focused scatter/bubble renderer.

## Tests

5 core unit tests: renders `[x,y]` pairs (one `<circle>` per point, correct
`width`); category objects produce a **legend + distinct palette colours** (point
count + legend swatches counted, both category labels and two palette colours
present); `size` **scales the radius** between the documented 3.0 and 14.0 bounds;
axis tick labels are present (6 ticks, correct mid value); and errors on empty /
non-array / malformed-point input. Plus the block drift-guard schema test. **CLI
verified** end-to-end (a JSON dataset → a valid `<svg>…</svg>`). `wafer build`
instantiates the chat block.

# pie-donut-chart-svg — competitor analysis (2026-07-30)

Tool function: generate a pie or donut chart as a standalone SVG from labeled
values, with auto-computed percentage slices, an optional legend, a title, and a
custom color palette. Pure-Rust, browser-local, nothing uploaded.

## Competitor scan (paraphrased — no copy/branding reproduced)

Skimmed the top real competitors returned by a web search for online pie/donut
chart makers:

1. **MiniWebtool — Pie Chart Maker** (miniwebtool.com/pie-chart-maker)
   — custom labels/colors, percentage labels, a donut mode, exploded slices, a
   legend, and PNG/SVG download.
2. **PieChartMaker.cc — donut mode** (piechartmaker.cc/donut-pie-chart)
   — a single "Hole Size" slider drives the pie↔donut continuum: 0 = a full pie,
   values between 0 and 1 = a donut; PNG/JPEG/SVG download.
3. **ScatterPlotMaker — Donut Chart** (scatterplotmaker.com/donut-chart)
   — percentages auto-computed from the sum of values; PNG/JPEG/SVG export.
4. **Make-Charts.com — Pie Chart Maker with Percentages**
   — percentage-labeled pies and donuts; SVG/PNG download, copy, embed link.
5. **makepiechart.com** — real-time preview as you type, high-res PNG + scalable
   SVG export.

(All reachable; none substituted.)

## Table-stakes params / defaults / UX patterns

| Capability | Typical default | In/out of model | Decision |
|---|---|---|---|
| Label + value data entry | one pair per line | in-model | `data` (required); accepts `Label,value` / `Label: value` / `Label = value` per line |
| Auto-computed percentages from the sum | on | in-model | percentages always computed; `show_percentages` toggles the on-slice label |
| Pie vs donut | pie | in-model | `chart_type` = `pie` \| `donut` (`Param::enumv`) |
| Donut hole size (inner-radius ratio) | ~0.5 | in-model | `donut_hole` 0.0–0.9, default 0.55 (donut only) |
| Legend listing each series | on | in-model | `show_legend` (default true) |
| Custom colors / palette | auto palette | in-model | `colors` (comma-separated CSS colors); a built-in 10-color palette otherwise |
| Chart title | none | in-model | `title` |
| Canvas size | ~square + legend room | in-model | `width` (default 640), `height` (default 400) |
| Show raw values (not just %) | off | in-model | `show_values` (default false) — adds the value into legend labels |
| Exploded / offset slices | off | in-model but low-value | considered, rejected — schema/UX bloat for a static SVG; a legend + % labels already read clearly |
| PNG / JPEG raster export | — | out-of-model | out — this tool emits an SVG string; raster export is the separate `svg-to-png` tool (chain them) |
| Real-time animation | — | out-of-model | out — the output is a single static SVG string |
| Embed link / hosted image | — | out-of-model | out — needs a backend/host; gizza is browser-local, no server |
| AI "describe it" generation | — | out-of-model | out — no model in a pure-Rust wasm block |

## Build decisions

- Input parsing is forgiving: split on newlines (also `;`), each entry is
  `label <sep> value` where the separator is the last `,`, `:` or `=`; non-numeric
  or non-positive values are rejected with a clear error naming the bad line.
- Slices are drawn clockwise from 12 o'clock as SVG arc paths; a `donut` renders
  the same geometry with an inner-radius cutout (even-odd path) sized by
  `donut_hole`.
- Percentages are rounded to 1 decimal and drawn at the slice mid-angle when the
  slice is wide enough to be legible; tiny slices are left to the legend.
- Legend sits to the right of the chart; each row is a color swatch + label,
  optionally the raw value and the percentage.
- All in-model table-stakes land in the descriptor; out-of-model items are listed
  above, not built.

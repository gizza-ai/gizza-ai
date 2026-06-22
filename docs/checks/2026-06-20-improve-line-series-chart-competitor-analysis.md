# line-series-chart — competitor analysis (2026-06-20)

Thirteenth `/create-next-tool` backlog pick (jpg-to-png was skiplisted before it
as a dup of image-convert). Pure-Rust tool (no deps — hand-rolled SVG). Output is
an SVG chart wrapped as an image/svg+xml envelope (like blocks/vectorize), so
surfaces are chat + CLI (no page mode for image-bytes output). Research via
`WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| MiniWebtool | multi-series, smooth/step lines, markers, area fill, themes; PNG/SVG | capabilities |
| Kanaries / StatsCharts | CSV/TSV input, multiple series, trend lines, dates; PNG/SVG | capabilities |
| MonoCalc / TheToolApp | title, axis labels, line styles, gridlines, markers, smoothing; PNG/SVG | capabilities |

## Gap diff vs our tool
Our tool: parses one or more numeric series (newline/`;` separated; comma/space
values), renders an SVG with axes, y min/mid/max labels + gridlines, a colored
polyline per series, a legend for multiple series, and an optional title. Covers
the multi-series + title + SVG-export core.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **PNG output** ("SVG or PNG" in the row) — needs an SVG rasterizer (e.g. resvg)
  or a bitmap chart backend; heavier dep, so SVG ships first and PNG is a follow-up.
- **X-axis labels / categories** — accept an optional labels list (we currently
  use the value index as x).
- **Point markers, area fill, smooth/step lines, custom series names + colors** —
  straightforward SVG additions.
- **CSV/TSV ingestion + headers** — could reuse csv-json-convert's parsing.

**Out-of-model:** interactive/zoomable D3-style charts (that's client JS, not a
static render), live theming UI.

## Tested
unit (6: single-series SVG, multi-series distinct colors + legend, semicolon/space
parsing, title XML-escaping, flat-series no-divide-by-zero, non-numeric/empty
errors) + drift-guard · `wafer build` validates the block (pure-Rust → also works
in the chat SW) · CLI renders a 2-series chart to a valid `<svg>` (1255 bytes) +
non-numeric error path. No page surface.

> Original work only — no competitor copy, branding, or trademarks copied.

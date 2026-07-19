# csv-chart-generator — competitor analysis (2026-07-17)

Tool: turn a chosen pair of CSV columns into a **bar, line, scatter, or
histogram** chart, rendered as a standalone **SVG**. Pure-Rust, runs on every
backend (chat SW, CLI, page); nothing is uploaded. All notes below are
paraphrased from public product pages — no competitor copy, branding, or
trademarks are reproduced.

## Competitors scanned

1. **Kanaries CSV to Chart** — drag columns onto a canvas to build bar/line/pie/
   scatter charts; download as PNG or SVG; copy an embed snippet.
2. **Sequel CSV Data Visualizer** — paste/upload a CSV, pick chart type
   (bar/line/scatter/pie/histogram); in-browser processing; export PNG/SVG.
3. **CSV to Graph Online (qingyanglabs)** — paste or upload CSV → line/bar/
   scatter; export PNG or SVG; emphasises "pixel-perfect scalable" output.
4. **DataPlotter** — scatter/line/histogram/box/heatmap from pasted data; export
   high-res PNG/SVG; share an interactive link.
5. **Graph Maker (graphmaker.org)** — pie/bar/line/area/scatter/histogram/box;
   export PNG/JPEG/SVG/PDF.

## Table-stakes (every serious competitor has these)

| Capability | Decision | Where it lands |
|---|---|---|
| Paste raw CSV text | **in-model** | `csv` param (multiline) |
| Choose chart type (bar/line/scatter/histogram) | **in-model** | `chart_type` enum |
| Choose which column is X | **in-model** | `x_column` (name or 1-based index) |
| Choose which column is Y | **in-model** | `y_column` (name/index; ignored for histogram) |
| Chart title | **in-model** | `title` |
| Custom size (width/height) | **in-model** | `width`, `height` (sliders) |
| Custom series colour | **in-model** | `color` (colour picker) |
| Scalable SVG output | **in-model** | text/SVG output |
| Histogram bin count | **in-model** | `bins` |
| Axis labels | **in-model** | derived from the chosen column names |
| Header-row detection | **in-model** | first non-numeric row treated as header |

## In-model — shipped in the descriptor

- `csv` (required, multiline) — raw CSV, comma-delimited, optional header row.
- `chart_type` = `bar | line | scatter | histogram` (enum).
- `x_column` — column name (from the header) or a 1-based index.
- `y_column` — column name/index; required for bar/line/scatter, ignored for
  histogram (which bins a single column: `x_column`).
- `title` — optional heading drawn above the plot.
- `width`, `height` — SVG pixel size (sliders, sane clamps).
- `color` — series/bar/point/bar colour (colour picker; any CSS colour).
- `bins` — histogram bucket count (2–100).
- Output is standalone SVG markup as text — copy, save `.svg`, or embed.

## Out-of-model (listed, deliberately not built)

- **PNG/JPEG/PDF export** — this repo's charts emit vector SVG; users can pipe
  the SVG into the existing `svg-to-png` / `svg-to-pdf` tools for a raster/PDF.
- **Interactive tooltips / zoom / shareable live links** — output is a static
  SVG, not an app; interactivity is a hosted-app feature, not a pure transform.
- **Pie / area / box-plot / heatmap chart types** — out of this tool's scope;
  gizza already ships `heatmap-chart`, and pie/box are separate shapes. Keeping
  to the four requested types (bar/line/scatter/histogram) keeps the descriptor
  focused.
- **Drag-and-drop column-mapping canvas** — a UI affordance; the equivalent
  capability (choosing X/Y columns) is exposed as plain `x_column`/`y_column`
  params, which work identically across chat, CLI, and the page.
- **Multiple Y series / grouped or stacked bars** — single-series keeps the
  schema unambiguous; a multi-series variant would be a separate tool.

## Not duplicates of existing gizza tools

`scatter-chart`, `line-series-chart`, `candlestick-chart`, `heatmap-chart` each
take a pre-shaped `data` string of one specific shape and render one chart kind.
This tool is CSV-column-driven (pick X/Y from arbitrary CSV) with a `chart_type`
switch spanning bar+line+scatter+histogram — a distinct, more general entry
point, so it is not a semantic duplicate of any single existing chart block.

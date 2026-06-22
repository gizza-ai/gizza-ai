# table-heatmap — competitor analysis & improvement snapshot (2026-06-22)

## What the tool does

Applies spreadsheet-style **color-scale conditional formatting** to a CSV/table and
returns a styled HTML `<table>` with shaded numeric cells. Each cell that parses as a
number (tolerating thousands commas, `$`/`€`/`£`, trailing `%`, and accounting-style
`(parentheses)` negatives) is shaded by its value on the chosen color scale; text cells
and the header row stay plain. Surfaces: chat (LLM), CLI (`gizza tool table-heatmap`),
and an in-browser page (`/tools/table-heatmap/`, pure wasm). Distinct from the existing
`heatmap-chart` / `correlation-heatmap` / `animated-heatmap` blocks (those render a numeric
*matrix* as an SVG/GIF chart) and from `csv-to-table` (plain table, no color formatting).

## Top competitors surveyed

1. **Excel — Conditional Formatting › Color Scales** (Microsoft). 2- and 3-color scales;
   per-cell shading; Number/Percent/Percentile/Formula anchors for min/midpoint/max; default
   3-color midpoint = 50th percentile/median.
2. **Google Sheets — Format › Conditional formatting › Color scale.** Min/Mid/Max points each
   with a data type (number/percent/percentile) and color.
3. **DataVizKit Heatmap Generator** (datavizkit.com) — free online; import CSV/XLSX or paste,
   customizable colors, live preview, image download.
4. **Bricks Heatmap Maker** (thebricks.com) — CSV→heatmap, auto palette, PNG/SVG export, live embed.
5. **CleanChart** (cleanchart.app) — color scale + labels + axis titles + cell annotations,
   export PNG/SVG/PDF.

## Capability diff & gap ranking (fit-to-model)

| Capability | Competitors | gizza before | Action |
|---|---|---|---|
| Per-cell color-scale shading of numeric cells | all | yes | kept |
| 2-color (sequential) scales | Excel/Sheets | yes (green, blue) | kept |
| 3-color (diverging) scales | Excel/Sheets | yes (RdYlGn, GnYlRd, BWR) | kept |
| Direction reversal (good↔bad) | Excel/Sheets | yes (RdYlGn + GnYlRd) | kept |
| Per-column vs whole-table scaling | (manual in sheets) | yes | kept |
| Tolerant number parsing ($, %, commas, () negatives) | partial | yes | kept (a differentiator) |
| Auto text-contrast (black/white on cell) | Excel | yes | kept |
| **Fixed min / max anchors** | Excel/Sheets (Number type) | **no — data range only** | **ADDED `min`, `max`** |
| **Fixed midpoint for diverging scales** | Excel/Sheets default 50th pct, settable | **no — always (min+max)/2** | **ADDED `midpoint`** |
| Header row left unshaded | Excel (manual) | yes | kept |

### Gaps closed this pass (in-model)

- **`min` / `max`** optional fixed bounds — pin the low/high ends of the scale instead of the
  data range (matches Excel/Sheets "Number"-type anchors). When both are set, `per_column` is
  moot and every column shares the fixed range. Validates `min < max`.
- **`midpoint`** optional fixed midpoint for the diverging scales (mapped to the neutral middle
  color via a piecewise-linear normalization that anchors the midpoint at t=0.5). Ignored for
  sequential scales. Default remains the data midpoint `(min+max)/2`.

These were the only meaningful capability gaps that fit gizza's pure-Rust, single-input,
recompute-on-change model.

## Out-of-model (deliberately NOT built)

- **PNG / SVG / PDF image export** (DataVizKit/Bricks/CleanChart). gizza emits an HTML `<table>`
  with inline styles, which is directly pasteable and editable — strictly more useful for the
  "format a table" use case than a flat image, and image-bytes output has no page render mode.
- **Drag-and-drop file upload / XLSX import.** The page is field/textarea based; XLSX is a separate
  concern already covered by the `xlsx-to-csv` block (chain the two).
- **Live data refresh / embeddable widgets** (Bricks). Out of scope for a stateless local tool.
- **Axis titles / chart annotations / legends** (CleanChart) — those belong to the chart-style
  `heatmap-chart` block, not a table formatter.
- **Percentile / formula anchor types** (Excel). Possible future in-model addition, but lower value
  than fixed-number anchors and adds parameter surface; deferred.

## Verification (all surfaces, post-improvement)

- `cargo test --workspace` — 15 core unit tests + 1 chat-schema drift-guard test pass.
- `wafer build` — chat `block.wasm` instantiates and validates (350 KiB).
- `wasm-pack build … web` — page wasm builds; generator renders `/tools/table-heatmap/`.
- CLI — `gizza tool table-heatmap` verified for default scale, enum scales, global vs per-column,
  fixed min/max, and the bad-scale error path.
- Playwright (`tool-page-table-heatmap.spec.ts`, 3 tests) — header + numeric shading, blue scale
  anchor, and fixed min/max bounds all pass headlessly under xvfb.

## Sources

- Excel color scales: https://www.ablebits.com/office-addins-blog/color-scales-excel/
- Microsoft conditional formatting: https://support.microsoft.com/en-us/office/use-conditional-formatting-to-highlight-information-in-excel-fed60dfa-1d3f-4e13-9ecb-f1951ff89d7f
- Google Sheets color scale: https://infoinspired.com/google-docs/spreadsheet/color-scale-formatting-in-google-sheets/
- DataVizKit heatmap generator: https://datavizkit.com/heatmap-generator
- Bricks heatmap maker: https://www.thebricks.com/heatmap-maker
- CleanChart: https://www.cleanchart.app/blog/how-to-create-heatmap

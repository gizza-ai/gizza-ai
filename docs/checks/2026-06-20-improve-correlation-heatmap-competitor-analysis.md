# correlation-heatmap — competitor analysis (2026-06-20)

Twenty-third `/create-next-tool` backlog pick. Pure-Rust (no deps — stats + SVG)
tool; output is an image/svg+xml envelope (like line-series-chart), so surfaces
are chat + CLI (no page mode for image-bytes output). Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| seaborn/pandas-style web "correlation matrix" tools | Pearson/Spearman, color heatmap, annotated cells, labels | capabilities |
| stats calculators | upload CSV, choose method, download PNG/SVG | capabilities / UX |

## Gap diff vs our tool
Our tool: parse numeric rows (columns = variables), compute a Pearson (linear) or
Spearman (rank, ties-averaged) correlation matrix, and render a labeled SVG
heatmap with a diverging blue→white→red scale and the value in every cell. Covers
the core (both methods + annotated, labeled heatmap).

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **PNG output** — needs an SVG rasterizer; SVG ships first (same call as
  line-series-chart's deferred PNG).
- **Header-row auto-detect** — accept a first row of column names instead of the
  separate `labels` param (could reuse csv parsing).
- **Kendall's tau** as a third method.
- **Color-scale legend** swatch.

**Out-of-model:** interactive hover/tooltips (static render), significance/p-values
overlay (could add, more stats).

## Tested
unit (6: perfect +1 / −1 Pearson, Spearman=1 for a monotonic non-linear y=x^3,
SVG has cells+labels+title, diverging color endpoints #ff0000/#0000ff/#ffffff,
parse errors for <2 rows / <2 cols / non-numeric / ragged) + drift-guard ·
`wafer build` validates the block (pure-Rust → also works in the chat SW) · CLI
renders a 3-column heatmap to a valid `<svg>` + error path. No page surface.

> Original work only — no competitor copy, branding, or trademarks copied.

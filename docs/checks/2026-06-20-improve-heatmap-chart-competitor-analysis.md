# heatmap-chart — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/heatmap-chart` — render an arbitrary numeric grid (CSV/matrix)
as a color-coded SVG heatmap with optional per-cell values, row/column labels, and
a min→max color legend. Chat + CLI (no page: SVG image-bytes output, like
correlation-heatmap / line-series-chart).

## Relationship to correlation-heatmap

Distinct tool: `correlation-heatmap` first *computes* a symmetric Pearson/Spearman
correlation matrix from observations and renders it on a fixed diverging
(blue↔red, −1..+1) scale. `heatmap-chart` visualizes the **raw values** of any
M×N grid on a sequential min→max colormap — the general-purpose heatmap.

## What competitors do

- **Spreadsheet conditional formatting** (Excel/Sheets color scales) — ubiquitous
  but locked inside a spreadsheet; not a shareable image or an API.
- **Online heatmap generators** (displayr, rapidtables, charts builders) — upload
  data, get a heatmap. Strengths: interactive. Weaknesses: data is **uploaded**,
  many export only raster PNG, and most need an account for export.
- **Python (matplotlib/seaborn `heatmap`)** — powerful but needs a runtime + code.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (no deps) compiled to wasm:
   runs in the chat Service Worker and headless via the CLI. Data never leaves the
   device.
2. **Scalable SVG output**, not a raster — crisp at any zoom, tiny, and editable;
   drops straight into web pages, docs, and slides.
3. **Self-contained legend.** A vertical min→max color bar with numeric endpoints
   is drawn alongside the grid, so the colors are interpretable without external
   context.
4. **Readable cell labels.** Optional per-cell values with an automatic
   black/white text color chosen by cell luminance, so numbers stay legible on
   both light and dark cells.
5. **Sensible colormap.** A perceptually-ordered RdYlBu-reversed sequential ramp
   (blue→pale-yellow→red) — a widely recognized heat scale — with graceful
   handling of a constant grid (all cells → mid color, no divide-by-zero).
6. **Flexible input + labels.** Comma/space/tab-separated rows; optional
   `col_labels`/`row_labels` (default `c1..`/`r1..`); optional title.

## Honest scope

- Single sequential colormap (no diverging/centered option yet — use
  correlation-heatmap for a centered −1..+1 scale).
- Linear min→max normalization (no log scale or manual vmin/vmax yet).

## Tests

6 core unit tests: colormap endpoints (blue/yellow/red at t=0/0.5/1), renders a
grid with values + legend, hides values when `show_values=false` (non-extreme
cell numbers absent), custom row/column labels appear, error cases (empty/ragged/
non-numeric), and a constant grid maps to the mid color without panicking. Plus
the block drift-guard schema test. CLI verified end-to-end on a 3×3 grid with
labels + title → a valid `<svg>…</svg>` (3.7 KB) containing the title, all
labels, cell values, and the expected min-cell-blue (`#2c7bb6`) /
max-cell-red (`#d7191c`) colors.

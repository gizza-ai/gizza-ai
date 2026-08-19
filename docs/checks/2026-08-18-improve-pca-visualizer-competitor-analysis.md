# pca-visualizer — competitor analysis (2026-08-18)

Scan run during implementation continuation after the tool was picked. Search query: `PCA t-SNE visualizer online scatter plot colored by labels CSV tool`; top real tools/pages were opened or inspected. Notes are paraphrased; no competitor copy, branding, screenshots, or marks are reused.

## Competitors reviewed

| # | Tool/page | Shape | Reachable |
|---|-----------|-------|-----------|
| 1 | Keptune PCA Calculator for Excel & CSV | Upload CSV/Excel, run PCA, visualize clusters, inspect loadings, prompt-style presets | yes |
| 2 | Karpathy t-SNE CSV web demo | Browser CSV paste, delimiter field, label/group text boxes, learning-rate and perplexity controls, run/stop buttons | yes |
| 3 | Yellowbrick t-SNE visualizer docs | Python/scikit visualizer: optional pre-PCA/SVD, target labels, scatter plot focus | yes |
| 4 | Partek PCA/UMAP/t-SNE scatter plots | Workflow scatter visualizations colored by sample attributes | yes (context only: suite product, not a paste-in tool) |

## Table-stakes capabilities and decisions

| Capability | Seen in | Our decision |
|---|---|---|
| Accept CSV/table input | 1, 2 | **in-model — built** (comma/tab/semicolon/pipe/whitespace table parser) |
| Header row detection | 1, 2 | **in-model — built** |
| PCA projection to 2D scatter | 1, 3, 4 | **in-model — built** (reuses existing deterministic PCA core) |
| t-SNE projection to 2D scatter | 2, 3, 4 | **in-model — built** (deterministic PCA-seeded implementation, capped) |
| Colour points by class/label/group | 2, 3, 4 | **in-model — built** (`label_column`, auto-detect, legend) |
| Explicit label column controls | 2 | **in-model — built** (name or 1-based index) |
| Perplexity and learning-rate controls | 2, 3 | **in-model — built** with bounded sliders |
| Iteration count / convergence control | 2, 3 | **in-model — built** |
| Standardize/scale numeric columns | 1, 3 | **in-model — built** (`scale`, default true) |
| SVG plot output | 1, 2, 3 | **in-model — built** (standalone SVG markup) |
| Coordinate export for reuse | 1, 3 | **in-model — built** (CSV and JSON) |
| Example presets | 1 | **in-model — built** (`[[example]]` chips) |
| Biplot loading arrows | 1 | **out-of-model for this tool**: the sibling `principal-component-analysis` already reports loadings; this tool focuses on point projection/cluster visualization. |
| UMAP | 4 | **out-of-model**: not in the picked row, would require a separate nearest-neighbour/graph optimizer tool. |
| Interactive pan/zoom/hover selection | 1, 4 | **out-of-model** for the static generated page; SVG/CSV/JSON exports cover portable output. |
| Stop/resume long optimization | 2 | **out-of-model** in the one-shot gizza tool model; instead t-SNE rows/iterations are capped and deterministic. |

## UX control choices

- Use a multiline table input with an iris-like preset because every reviewed peer starts from a table or CSV.
- Keep PCA as the default: fastest, deterministic, interpretable axis captions.
- Expose t-SNE controls only as bounded sliders/number boxes: perplexity 1–100, iterations 50–2000, learning-rate 1–1000.
- Keep `scale` enabled by default to match common PCA/t-SNE preprocessing guidance for mixed-unit columns.
- Return SVG by default so a user gets the visual artifact immediately; CSV/JSON support downstream plotting and exact tests.

## Copy / branding

No competitor wording, names, UI copy, screenshots, or assets are reused. Page prose and examples are original and generic.

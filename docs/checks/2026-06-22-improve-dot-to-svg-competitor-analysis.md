# dot-to-svg — competitor analysis (2026-06-22)

Tool: render Graphviz DOT diagram source into a standalone, scalable SVG, fully
in-browser (pure-Rust `layout-rs` layout engine — no Graphviz install, nothing
uploaded). Surfaces verified: chat block (`wafer build` validate/instantiate OK,
463 KiB), CLI (`gizza tool dot-to-svg`), standalone page (Playwright, 3/3).

## Surface verification (Phase 1)

- **Chat / LLM API:** `wafer build` validated + instantiated the block; schema
  drift-guard unit test passes (`dot` required, `dark_mode` boolean default
  false).
- **CLI:** `gizza tool dot-to-svg dot='digraph { a -> b; b -> c; a -> c; }'`
  returns SVG markup; `dark_mode=true` introduces light strokes (`#e6e6e6`).
- **Page:** `/tools/dot-to-svg/` renders DOT → SVG text; digraph, undirected
  graph, and dark-mode toggle all covered by Playwright (`tool-page-dot-to-svg.spec.ts`).

## Competitors scanned

1. **Graphviz Online (dreampuf.github.io/GraphvizOnline)** — WASM `viz.js`,
   real-time preview, engine selector (dot/neato/fdp/…), output formats
   svg/png/json, syntax-highlighted editor.
2. **graphvizonline.net** — editor + live preview, multiple engines + formats,
   shareable URLs.
3. **sketchviz.com** — hand-drawn-style rendering of DOT graphs.
4. **magjac/d3-graphviz** — D3 + viz.js, animated transitions between graphs.
5. **mischnic/dot-svg** — browser DOT→SVG renderer component (Hpcc-js wasm).

## Gap analysis (fit to gizza's pure-Rust browser-local model)

| Competitor feature | In gizza model? | Action |
|---|---|---|
| DOT → SVG, directed + undirected | yes | shipped |
| Node/edge labels, chained edges | yes (layout-rs) | shipped |
| Browser-local, no install, no upload | yes (core strength) | shipped + messaged in copy |
| Dark / theme recolor | yes (post-process) | **added** `dark_mode` (not common in competitors — value-add) |
| Live preview / syntax highlight | partial | page recomputes on input; editor highlighting is a site-shell concern, out of the tool's compute scope |
| Engine selector (neato/fdp/circo/twopi) | **no** | `layout-rs` only implements the hierarchical (dot) layout; alternate engines are out of model |
| PNG / PDF raster output | **no** | rasterizing is a separate concern — the existing `svg-to-png` tool converts this tool's SVG output; keeping dot-to-svg single-purpose |
| JSON / xdot / plain output | **no** | niche debugging formats; not exposed by `layout-rs` |
| Animated transitions (d3) | **no** | runtime animation, not a static-render tool concern |

## Decisions

- Closed every **in-model** gap: directed + undirected graphs, labels, chained
  edges, and a dark-mode theme toggle (a differentiator most competitors lack).
- Out-of-model features (alternate layout engines, PNG/JSON output, live
  animations) are documented above and intentionally not built — `layout-rs`
  exposes only the hierarchical layout and SVG output, and rasterization is
  already covered by the sibling `svg-to-png` tool.
- No competitor copy, branding, or trademarks were used.

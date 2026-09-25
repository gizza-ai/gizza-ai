# radar-chart — competitor analysis (2026-08-29)

Scan run **before** implementation, per the `/improve-tool` Phase 2–3 procedure applied to a
new build. One web search for "online radar chart maker / spider chart generator", then the top
real competitors were skimmed directly. Everything below is **paraphrased**; no competitor copy,
branding, wording, or assets were reused.

## Competitors reviewed

| # | Competitor | What it is | Notes |
|---|------------|------------|-------|
| 1 | graphmaker.org radar chart | Free browser chart builder, "serverless"/local rendering | Comma-separated labels + values, heavy styling panel, PNG/JPG/SVG export |
| 2 | radarchart.io | Dedicated radar/spider builder, no signup, client-side | Attribute rows + per-item score sliders, multi-item overlay, PNG/SVG, shareable config links |
| 3 | Better Analyst (formerly Formula Bot) chart maker | AI-assisted chart generator, paste/upload data | Auto-detects variables/categories, prescriptive guidance on axis count and normalization |
| — | Chart.js radar docs (reference implementation, not a competitor site) | The de-facto option vocabulary everyone else mirrors | `scales.r` min/max/ticks, `angleLines`, `pointLabels`, `fill`, `pointRadius`, `tension` |

Two further hits (Canva, an image-online PHP generator) were not usable as evidence — Canva is a
sign-in design suite rather than a comparable single-purpose tool, and the PHP generator returned
HTTP 403 to a plain fetch. Noted honestly rather than padded.

## Table stakes observed (params · defaults · UX)

| Capability | Seen at | In/out of model | Decision |
|---|---|---|---|
| Paste a labels row + one value row | 1, 3 | in | `data` accepts a header row of axis names + one row per series |
| Multiple overlaid series (2–5 items) | 2, 3 | in | Wide table (`series,Axis1,Axis2,…`) and long/tidy `series,axis,value` both parse |
| Axis (spoke) labels | all | in | Drawn around the polygon; `show_axis_labels` |
| Scale min / max, "start axes at zero" | 2, 3, Chart.js `suggestedMin/Max` | in | `scale_min` / `scale_max`, default auto with a zero-based floor |
| Per-axis independent scaling (mixed units) | 3 (as "normalize to a common range") | in | `scale = shared \| per_axis \| percent` — competitors tell users to normalize by hand; doing it in-tool is a real gap closed |
| Ring / gridline count | 1, 2, Chart.js ticks | in | `rings` (0–10, default 5) |
| Grid shape: polygon web vs concentric circles | 1, 2 | in | `grid_shape = polygon \| circle` |
| Spokes / angle lines toggle | Chart.js `angleLines` | in | `show_spokes` |
| Semi-transparent fill for overlap readability | 2, 3 | in | `fill_opacity`, default 0.25 |
| Line width | 1, Chart.js `borderWidth` | in | `line_width`, default 2 |
| Point markers + size | 2, Chart.js `pointRadius` | in | `point_radius`, 0 disables |
| Value labels at each vertex | 2 (hover reveal) | in | `show_values` prints the number at each vertex |
| Ring tick labels (scale numbers) | 1, Chart.js ticks | in | `show_ticks` |
| Legend + placement | 1, 2 | in | `legend` (default true, radar charts are comparison charts); placement kept to one clean strip rather than 4 positions × 3 alignments |
| Chart title | 1, 2 | in | `title` |
| Palette / per-series colours | all | in | `palette` (6 schemes) + `colors` override list |
| Background colour, light/dark theme | 1 | in | `background`, `theme` |
| Canvas width/height | 1 | in | `width`, `height` |
| SVG export | 1, 2 | in | Default output is self-contained SVG |
| Hover tooltip showing the exact value | 2 | in (partial) | Native SVG `<title>` on every vertex marker — works in any browser, no JS |
| Shareable link that encodes the whole config | 2 | in (platform) | The generated page already round-trips every field through `?param=` query strings |
| PNG / JPG raster export | 1, 2 | **out** | Needs a rasterizer; this block is pure Rust with no image encoder. SVG is lossless and converts anywhere. Considered, not built. |
| CSV/XLSX file upload | 3 | **out** | Page input is a paste field; file upload belongs to the ffmpeg/file-input tool shape. Paste covers the same data. |
| Natural-language "describe your chart" AI | 3 | **out** | Needs a model/server; out of the browser-local pure-wasm model. (The chat surface covers this need at the platform level.) |
| Live drag-to-score sliders per cell | 2 | **out** | Would need a bespoke grid editor; the declarative form + paste field is the platform control set. Considered, rejected. |
| Font family picker, legend position × alignment matrix | 1 | in but **rejected** | Schema bloat for marginal gain; the chart uses a system-font stack that renders consistently everywhere. |

## Guidance competitors publish (folded into our page copy, in our own words)

- 3–8 axes is the readable range; below 3 a radar is not meaningful, above ~10 it turns into a
  scribble. Our tool enforces a minimum of 3 axes and caps at 60, and says so on the page.
- 2–4 overlaid series is the practical limit before the polygons stop being separable. Our page
  states this and the fill opacity default is tuned for it.
- Axes should start at zero and share a scale, or the shape misleads. We default to a zero-based
  shared scale and offer `per_axis`/`percent` explicitly for genuinely mixed units.

## Gaps closed that competitors do NOT ship

- `scale = per_axis` and `scale = percent` — every competitor tells the user to normalize their
  data by hand first; this does it in the tool.
- `summary` and `json` outputs — an aligned text table of every series/axis value with its
  normalized radius, and machine-readable vertex geometry. No competitor exposes the computed
  geometry.
- Deterministic output: identical input always produces byte-identical SVG, so charts can be
  committed to a repo and diffed.

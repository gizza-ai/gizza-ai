# collage-splitter — competitor analysis (2026-08-01)

**Tool:** `collage-splitter` — "Detects the cells of a photo-grid collage and exports each original photo back out individually."

**Type:** pure-Rust (`image` + `zip`), one image input → a ZIP of the individual cell images.
Runs on every backend including the chat Service Worker. Surfaces: **chat + CLI, no standalone
page** (a ZIP-of-images output fits neither the pure-text nor the ffmpeg media page shape — the
same "no-page file-input" pattern as `multi-photo-scan-splitter` / `extract-pdf-images`).

## Why this is NOT a duplicate of `multi-photo-scan-splitter`

`multi-photo-scan-splitter` targets a **flatbed scan** with photos placed at arbitrary positions and
angles on a scanner bed; it uses threshold + **connected-components + min-area-rectangle deskew** to
find and straighten each photo. `collage-splitter` targets a **regular photo-grid collage** (an
Instagram/MidJourney/grid-maker layout): uniform gutters/borders separating cells laid out on a
grid. It uses **gutter-line detection** (columns/rows that are mostly the border colour) to recover
the grid, or an explicit **rows × columns** even split — no connected components, no deskew. The
algorithm, the input assumption (grid vs. free placement), and the canonical competitor category
("grid image splitter") are all different.

## Competitor scan (paraphrased — no copy/branding reused)

Surveyed the common "grid image splitter / collage splitter / Instagram grid maker / MidJourney grid
splitter" category (imagesplitter.ai, splitimage.im, templated.io image splitter, Instagram grid
makers, MJ Splitter). Table-stakes observed:

| Capability | Competitor norm | Our decision |
|---|---|---|
| Split into N rows × M columns (equal) | Universal; grids up to ~20×20 (≤400 cells) | **In-model** — `rows`, `columns` (0 = auto-detect that axis), each capped at 20 |
| Auto-detect the grid / cells | Some tools; our headline feature per the description | **In-model** — gutter-line detection when rows/cols left at 0 |
| Output each piece as a separate file | Universal | **In-model** — one image per cell |
| Package all pieces as a single ZIP | Common (templated.io, IG makers) | **In-model** — always ZIP (`cell_1.png`, `cell_2.png`, …) |
| Auto-number pieces in reading order | Universal (1, 2, 3…) | **In-model** — row-major, zero-padded, `prefix` configurable |
| Formats JPG / PNG / WebP | Universal | **In-model** — `format` = png/jpeg/webp/bmp |
| Trim leftover border/gutter from each cell | Some | **In-model** — `trim` pixels inward per cell |
| Gutter/border colour handling | Implicit (white gutters usual); MidJourney grids have colour | **In-model** — `gutter` = auto/white/black for detection |
| 2×2 MidJourney preset | MJ Splitter | Achieved via `rows=2 columns=2` (documented) |
| Max upload ~50 MB | templated.io | Bounded to 24 MiB input + ~13 MP raster (wasm sandbox); actionable "re-export smaller" error |
| Drag-and-drop UI, per-cell adjust, live preview | Web UIs | **Out-of-model** — this repo ships chat + CLI only for a ZIP-of-images tool (no page surface); the browser tool UI lives in the private site repo |
| Login / cloud batch / watermark-free tiers | SaaS tools | **Out-of-model** — gizza is browser-local, no account, no server |

## UX patterns considered

Competitors expose a rows×cols numeric grid picker, a format dropdown, and a one-click ZIP download.
We map those to descriptor params (`rows`, `columns`, `format`) and always return a ZIP. Preset chips
(2×2, 3×3) are a page affordance; this tool has no page, so presets are documented as parameter
combinations in the skill description instead.

## Out-of-model (listed, not built)

- Interactive drag-to-adjust cell boundaries / live preview (needs the page UI, which this
  ZIP-of-images shape doesn't get in this repo).
- Cloud batch of many collages, accounts, paid watermark-free tiers (no server/login in gizza).

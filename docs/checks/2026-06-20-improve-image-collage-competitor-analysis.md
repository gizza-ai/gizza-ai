# image-collage — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/image-collage` — combine several images into one collage
(grid, horizontal strip, or vertical stack) as a PNG. Chat + CLI (multi-source
array input + image-bytes output, like images-to-pdf — no page).

## What competitors do

- **Online collage makers** (canva, fotor, befunky, photojoiner, kapwing) —
  drag images into a layout, export. Strengths: rich templates, drag UX.
  Weaknesses: images are **uploaded to a server** (privacy + size caps),
  watermarks/paywalls on free tiers, accounts required for export.
- **ImageMagick `montage`** — the reference CLI (`montage *.png -tile 3x out.png`)
  — fully local and powerful, but requires installing ImageMagick and learning
  its flags.
- **Python (PIL)** — paste images onto a canvas; needs a runtime + code.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`image` crate) compiled to
   wasm: runs in the chat Service Worker and headless via the CLI. Images never
   leave the device.
2. **Three layouts in one call** — `grid` (auto roughly-square, or set
   `columns`), `horizontal` (one row), `vertical` (one column).
3. **Aspect-ratio preserved, no distortion.** Each image is scaled to *fit* a
   uniform cell (not stretched) and centered, with the background showing in the
   letterbox area — so mixed sizes/orientations compose cleanly.
4. **Mixed input formats.** PNG/JPEG/WebP/GIF/BMP are all decoded and unified
   into one PNG (verified with a PNG + GIF input).
5. **Configurable gap + background.** `gap` adds even spacing between and around
   cells; `background` accepts `#rgb`/`#rrggbb`/`#rrggbbaa` (alpha supported),
   shown in gaps and letterbox padding.
6. **Chainable.** Each input is a `url` or a prior tool's `ref`, and the PNG
   output is itself a `ref` for downstream tools.
7. **Guard-railed.** Cells capped at 1200 px and the canvas at ~40 MP, so a huge
   batch fails cleanly instead of OOMing.

## Honest scope

- Uniform cell sizing (every cell is the same size). No masonry/Pinterest-style
  variable tiling, and no per-image captions/borders yet.
- PNG output only (lossless, alpha-capable) — no JPEG export option yet.

## Tests

7 core unit tests: layout parsing, color parsing (`#rgb`/`#rrggbb`/`#rrggbbaa`/
empty→white/invalid), grid-shape math (auto cols = ceil(sqrt n), explicit
columns, horizontal/vertical), exact output dimensions for a horizontal collage
(2×40 + 3×10 gap = 110 wide) and a 3-image grid (2×2, gap 0 → 40×40), decode +
build from PNG bytes, and error cases (empty list, undecodable bytes). Plus the
block drift-guard schema test. CLI verified over the wire combining a PNG (tux)
and a GIF (giphy) horizontally → a valid 542×276 PNG.

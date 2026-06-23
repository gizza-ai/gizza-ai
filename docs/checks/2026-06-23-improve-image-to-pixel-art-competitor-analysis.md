# image-to-pixel-art — competitor analysis & improvement check (2026-06-23)

## Tool summary
Turns a photo into retro, limited-palette pixel art. Pipeline:
1. **Downscale** to a coarse grid (each `pixel_size`×`pixel_size` source block → one art pixel) with a Triangle filter so each block is an averaged colour.
2. **Quantize** the small grid to ≤ `colors` with NeuQuant — an image-derived palette (the genuine pixel-art look, not a fixed palette).
3. **Upscale** back to ~original dimensions with nearest-neighbour so every cell becomes a crisp solid block.

Pure Rust (`image` + `color_quant`) → runs on every backend incl. the chat Service Worker.

- **Params:** `pixel_size` (2–64, default 8), `colors` (2–256, default 16), plus the standard `url`⊕`ref` image source.
- **Surfaces:** chat (LLM API) + CLI. **No page** — image-bytes output has no page render mode (same as `image-color-quantize`, `normalize-image`).
- **Output:** PNG via `build_media_envelope` (`image/png`).

## Surface verification (Phase 1)
- `cargo test --workspace` — 6 tests pass: drift-guard schema test + 5 core (palette reduction, blocky-grid uniformity, solid-stays-solid, clamping, error path).
- `wafer build` — block.wasm built + validated (1437.6 KiB), instantiates clean.
- `cargo install --path cli` + `cargo run … generator` — both succeed.
- CLI: `gizza tool image-to-pixel-art url=<live QR PNG> pixel_size=10 colors=4` → `pixel art (10px blocks, 4 colors, 4188 bytes PNG)`, exit 0, valid 300×300 PNG.

## Competitor landscape (top-5)
Researched the common pixel-art / "pixelate photo" web tools (generic feature sets; **no copy, branding, or trademarks copied** — capability comparison only):

1. **Generic "pixelator" web tools** (online-image-pixelator style) — single block-size slider, fixed output = input size. Often no palette control.
2. **Pixel-it / pixelit.js style libraries** — block-size + palette-size + optional fixed retro palettes (NES/GameBoy). Often a grayscale toggle.
3. **Photoshop "Mosaic" + "Posterize" combo** — separate cell-size and colour-level controls; the manual equivalent of this tool's two steps.
4. **8-bit / retro photo filters** — fixed small palettes (e.g. 16-colour EGA, GameBoy 4-green), dithering for gradients.
5. **AI pixel-art generators** — text/image → sprite; out of model (needs a generative model; gizza is pure-Rust).

## Gap analysis (fit-to-model)

| Capability | Competitors | This tool | Decision |
|---|---|---|---|
| Adjustable block / pixel size | yes | **yes** (`pixel_size` 2–64) | in-model ✓ |
| Adjustable palette size | some | **yes** (`colors` 2–256) | in-model ✓ |
| Image-derived optimal palette | few | **yes** (NeuQuant) | in-model ✓ — a differentiator vs fixed-palette tools |
| Crisp blocky output (nearest upscale) | yes | **yes** | in-model ✓ |
| Preserves dimensions | mixed | **yes** (~original via grid multiple) | in-model ✓ |
| Fixed retro palettes (NES/GameBoy) | some | no | **out of scope this iter** — would need curated palette tables + a `palette` enum; the image-derived palette already covers the core need. Noted, not built. |
| Dithering (Floyd–Steinberg) | some | no | **out of scope** — NeuQuant gives a clean limited-palette look; dithering is a distinct aesthetic and a separate tool candidate. Noted, not built. |
| Grayscale / sepia pre-pass | some | no | already covered by the existing `image-grayscale` tool (compose via `ref`). Not duplicated here. |
| AI sprite generation | some | no | **out of model** (needs a generative model). |

**Conclusion:** all in-model core capabilities (adjustable block size, adjustable image-derived palette, crisp blocky output, dimension preservation) are present and on par with or ahead of the mainstream pixelator tools. Fixed retro palettes and dithering are noted as possible future enhancements but are distinct aesthetics, not gaps in the core promise. Grayscale is already a separate composable tool. No copy/UX gaps to close for a no-page (chat + CLI) tool.

## Distinct from existing blocks
- `image-color-quantize` — reduces colours at **full resolution** (no downscale, no blocks). This tool adds the defining downscale→nearest-upscale blocky grid.
- `image-pixelate-censor` — only mosaics a **rectangular region** for censorship; no global palette reduction.
- Not a duplicate.

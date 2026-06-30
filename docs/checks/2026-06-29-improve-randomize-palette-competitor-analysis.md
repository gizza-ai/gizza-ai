# Competitor analysis: randomize-palette

Date: 2026-06-29
Tool: `gizza-ai/randomize-palette`

## Goal

Randomly remap an indexed GIF/PNG-8 image palette so hidden shapes or steganographic payloads that rely on near-identical palette entries become visible. Preserve the pixel-index structure, make the shuffle deterministic via `seed`, and return a PNG.

## Competitors reviewed

1. StegOnline / George Om
   - URL: https://www.georgeom.net/StegOnline/upload
   - Notes: Browser-based image steganography suite with bit-plane browsing, LSB extraction/embed, PNG chunk inspection, RGBA export, and palette browsing.
   - Relevant gaps: broader stego suite and interactive bit-plane analysis. Out of model for this single-purpose tool except palette-aware forensic framing.

2. Aperi'Solve
   - URL: https://www.aperisolve.com/
   - Notes: Automated steganalysis platform that runs multiple external tools such as zsteg, steghide, outguess, exiftool, binwalk, foremost, and strings.
   - Relevant gaps: many heavyweight external analyzers and server-side pipeline. Out of model for the pure Rust/WASM gizza block.

3. Online GIF Tools — Change GIF Color Palette / Replace GIF Color
   - URLs: https://onlinegiftools.com/change-gif-color-palette and https://onlinegiftools.com/replace-gif-color
   - Notes: GIF-specific palette editing/replacement tools; search snippets mention finding palette entries and randomly changing color indexes.
   - Relevant gaps: animation-aware GIF export and manual replacement controls. In model for future enhancement, but current gizza tool accepts GIF and emits a PNG snapshot/remap.

4. Online PNG Tools — Swap PNG Colors / Randomize PNG Pixels
   - URLs: https://onlinepngtools.com/swap-png-colors and https://onlinepngtools.com/randomize-png-pixels
   - Notes: PNG color replacement and pixel randomization utilities.
   - Relevant gaps: manual color-pair replacement and true pixel-position randomization. Pixel randomization would destroy stego geometry, so intentionally not copied.

5. Pixel Palette Swap
   - URL: https://pixelpaletteswap.com/
   - Notes: Palette swap focused on pixel art GIFs/images, previewing animations and exporting GIF/PNG sequences.
   - Relevant gaps: interactive palette editor, animation preview, PNG sequence export. Mostly UI/page work; this block is chat/CLI media output only.

## Fit-to-model decisions

Built in model:
- Pure Rust image decoding/encoding with `image` and `color_quant`.
- Deterministic seeded palette permutation so results are reproducible across chat and CLI.
- Exact color-palette path for <=256 distinct colors, matching indexed-image use cases.
- Quantization fallback for true-color images so the tool remains useful instead of failing.
- PNG media output envelope with clear filename and summary.

Intentionally not built / out of model:
- Server-side stego suites (`zsteg`, `steghide`, `binwalk`, `foremost`, etc.).
- Interactive manual palette editor or animation timeline UI.
- Bit-plane extraction, LSB embedding/extraction, and metadata forensics.
- Page surface: this is an image-bytes output tool, and existing loop guidance says image-bytes outputs have no page render mode.

## Verification snapshot

- `cargo test --workspace` in `blocks/randomize-palette/`: passed.
- `wafer build` in `blocks/randomize-palette/`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/randomize-palette/web --target web --release --out-dir pkg`: not applicable; this tool has no `web/` crate/page because it is image-bytes output.
- `cargo install --path cli`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed.
- `gizza list | grep randomize-palette`: listed the tool.
- `gizza tool randomize-palette url='https://api.qrserver.com/v1/create-qr-code/?data=randomize-palette-test&size=128x128' seed=42`: passed and wrote `randomized-palette.png`.
- Playwright page test: not applicable; no page surface.

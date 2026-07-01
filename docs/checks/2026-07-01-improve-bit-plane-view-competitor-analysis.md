# bit-plane-view competitor analysis (2026-07-01)

## Tool

`bit-plane-view` extracts a single bit plane from an image channel and renders it as a PNG. It targets steganography inspection, image forensics, watermark analysis, and low-level channel debugging.

## Competitor snapshot

1. StegoToolkit Steganography Analyzer
   - Positioning: browser-based steganography analysis with statistical checks and a bit-plane viewer.
   - Strengths: broad stego workflow, including analysis beyond simple bit-plane rendering.
   - Gaps vs gizza: gizza focuses the single operation into a deterministic chat/CLI tool with explicit channel, bit, and render-mode parameters.

2. StegOnline
   - Positioning: online image steganography embedding/extraction, commonly used for LSB workflows.
   - Strengths: interactive stego workbench for hiding/extracting data.
   - Gaps vs gizza: broader UI-driven workflow; gizza implements the reusable primitive of rendering one selected channel bit plane as an image artifact.

3. Aperi'Solve
   - Positioning: online steganography analysis platform with layer analysis and other forensics tools.
   - Strengths: many automated analyses and external-tool style checks.
   - Gaps vs gizza: server-style platform; gizza is a small local block/CLI operation with no broad automated analysis claims.

4. A.Tools Image Blind Watermark / LSB Viewer
   - Positioning: inspect least-significant-bit information per image channel.
   - Strengths: clear LSB/watermark-oriented use case.
   - Gaps vs gizza: gizza supports any bit plane 0..7, multiple render modes, and all channels including alpha and luminance gray.

5. TidyKit Steganography / LSB Extractor and MatrixPuzzle LSB Decoder
   - Positioning: extract hidden messages/data from least-significant bits.
   - Strengths: message-oriented LSB extraction workflows.
   - Gaps vs gizza: text extraction is separate from visual inspection; gizza intentionally renders the plane so visual patterns and embedded shapes can be inspected before choosing an extraction strategy.

## Gap decisions

Built / retained in-model:

- Image input via existing source resolver (`url` or `ref`) and PNG image output via media envelope.
- Channel selection: red, green, blue, alpha, and gray/luminance.
- Bit selection from 0 (LSB) through 7 (MSB).
- Render modes:
  - `binary`: set bit white, clear bit black, maximum contrast.
  - `weighted`: set bit rendered at its positional gray value (`1 << bit`).
  - `color`: set bit rendered in the selected channel's color (gray/alpha as white), clear bit black.
- Same-dimensions PNG output.
- Unit tests for parsing, channel/bit behavior, output PNG shape, and invalid inputs.
- Chat/CLI surfaces; no standalone page because this is image-bytes input/output and the existing page pattern does not support generic image upload/render for this block shape.

Out-of-model / deferred:

- Automated hidden-message extraction, zsteg-style scans, chi-square/RS/sample-pairs analysis, steghide/outguess/binwalk orchestration, or OCR of rendered planes.
- Multi-plane grid visualizations. This tool intentionally renders one precise plane for composable use.
- Server-side uploads or saved workspaces.

## Verification snapshot

- `cargo test --workspace` from `blocks/bit-plane-view/`: passed (1 block schema test + 11 core tests).
- `wafer build` from `blocks/bit-plane-view/`: passed and produced `target/block.wasm`.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed.
- `cargo install --path cli`: passed.
- CLI smoke with `gizza tool bit-plane-view url=https://dummyimage.com/4x4/ffffff/000000.png channel=red bit=7 mode=binary`: passed and wrote `red-bit7.png` with a PNG media summary.
- `wasm-pack` / Playwright page test: not applicable; this block has no web/page surface because it is image input + image bytes output.

# spritesheet-slice — competitor analysis & surface checks (2026-06-29)

**Tool:** `spritesheet-slice` — slice a grid sprite sheet into individual image frames and return a ZIP archive.

## Verification snapshot

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/spritesheet-slice && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 16 tests passed (core slicing/format/prefix/max-frame + descriptor drift guard) |
| Chat block | `cd blocks/spritesheet-slice && CARGO_BUILD_JOBS=1 wafer build` | ✅ `target/block.wasm` validated |
| Web/page | n/a | ZIP-of-images output has no current page renderer pattern; tool is chat + CLI like other file-input/file-output blocks |
| Page generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ generator completed; no standalone page expected |
| CLI | `CARGO_BUILD_JOBS=1 cargo install --path cli --force`; `gizza list`; `gizza describe spritesheet-slice`; `gizza tool spritesheet-slice url=… columns=2 rows=2 max_frames=3 prefix=tile format=png` | ✅ listed/described, wrote `image.zip`, reported 3 sliced frames |

## Competitor scan

Search query: `online spritesheet slicer split sprite sheet into frames grid PNG zip`.

Representative competitors and feature patterns:

1. **Ezgif Sprite Cutter** — grid-based sprite cutting with offset/spacing controls; exports common image formats and can download all frames as a ZIP.
2. **Split Image Online** — client-side privacy-focused splitter for PNG/JPG/WebP; supports grid splitting, naming/order options, and offline-friendly operation.
3. **Spritesheetcutter.pro** — rows/columns workflow with preview, download-all ZIP, output format choice, and advertised auto-detection.
4. **SpriteSheetCutter / SpriteSheetConverter-style tools** — browser-based grid slicers with rows/columns, preview, per-format export, and ZIP download.
5. **Sprite Sheet Slicer by isometric8** — desktop/CLI utility that supports PNG/JPG/BMP/GIF sprite sheets and output as PNG/JPG/BMP/GIF, plus command-line automation.

## Gap analysis

| Capability / UX pattern | Competitors | Implemented in gizza |
| --- | --- | --- |
| Rows + columns grid slicing | Common across web slicers | ✅ `columns` + `rows` |
| Fixed tile-size slicing | Desktop and advanced web tools | ✅ `tile_width` + `tile_height` |
| Margin/offset and spacing | Ezgif and advanced slicers | ✅ `margin` + `spacing` |
| ZIP download of all frames | Common | ✅ ZIP envelope (`application/zip`) |
| Output format selection | Ezgif / isometric8 / web slicers | ✅ `png`, `jpeg`, `webp`, `bmp` |
| Custom file naming | Split Image Online-style tools | ✅ `prefix` option with stable zero-padded names |
| Skip blank/transparent frames | Useful for padded sheets | ✅ `skip_empty` |
| Cap output count | Practical safety/preview option | ✅ `max_frames` |
| Browser visual preview | Common in dedicated web apps | Not built: current gizza page generator lacks a ZIP-of-images preview/download page pattern for this tool shape |
| Auto-detect sprite bounds/grid | Some competitors advertise it | Not built: heuristic-heavy and risky; explicit grid/tile parameters keep output deterministic |
| Animated GIF decompile / rebuild animations | Desktop tools / Ezgif adjacent flows | Out of scope: this tool slices static sprite sheets; animation assembly is a separate workflow |

## Notes

The in-model improvements from competitor review were added as descriptor/core capabilities rather than page UI: output format choice, custom prefix, max-frame cap, and skip-empty support. The tool remains local/pure-Rust, deterministic, and compatible with chat/CLI surfaces.

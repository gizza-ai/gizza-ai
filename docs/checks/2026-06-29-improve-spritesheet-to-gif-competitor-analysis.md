# spritesheet-to-gif — competitor analysis & surface checks (2026-06-29)

**Tool:** `spritesheet-to-gif` — slice a grid sprite sheet and combine the frames into one animated GIF.

## Verification snapshot

| Surface | Check | Result |
| --- | --- | --- |
| Core/API | `cd blocks/spritesheet-to-gif && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 9 tests passed (core GIF encoding + descriptor drift guard) |
| Wafer block | `cd blocks/spritesheet-to-gif && CARGO_BUILD_JOBS=1 wafer build` | ✅ `target/block.wasm` validated |
| Wafer fixture | `cd blocks/spritesheet-to-gif && CARGO_BUILD_JOBS=1 wafer test tests/spritesheet-gif-url.json` | ✅ fixture passed |
| Web/page | n/a | Image-bytes/GIF output is chat + CLI only; current generator has no standalone page renderer for this tool shape |
| Page generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ generator completed; no standalone page expected |
| CLI | `CARGO_BUILD_JOBS=1 cargo install --path cli --force`; `gizza list`; `gizza describe spritesheet-to-gif`; `gizza tool spritesheet-to-gif url=… columns=2 rows=2 max_frames=3 delay_ms=80 loop_count=0` | ✅ listed/described, wrote `image.gif`, reported 3 animated frames |

## Competitor scan

Search query: `online spritesheet to animated gif converter sprite sheet gif maker grid frames`.

Representative competitors and feature patterns:

1. **Online GIF Tools — Convert a Sprite Sheet to a GIF** — converts sprite sheets into animated GIFs with per-sprite width/height controls.
2. **Colliding Scopes Sprite Sheet to GIF Converter** — simple free browser tool for game creators and animators to turn a sprite sheet into a GIF.
3. **Ezgif GIF/sprite tooling** — adjacent conversion flows between animated GIFs and sprite sheets, with timing and frame manipulation patterns.
4. **Spritesheet Generator-style game tools** — arrange/upload frames and preview animation; useful for understanding animation preview/export expectations.
5. **Morrow Shore Sprite Sheet & Animation Converter** — supports sprite sheet to GIF/ZIP and GIF to sprite sheet, emphasizing no-account browser workflows.

## Gap analysis

| Capability / UX pattern | Competitors | Implemented in gizza |
| --- | --- | --- |
| Grid sprite sheet to GIF | Core flow of dedicated converters | ✅ `columns` + `rows` grid mode |
| Tile-width/tile-height mode | Common in sprite sheet converters | ✅ `tile_width` + `tile_height` |
| Offset/margin and spacing | Advanced sprite cutters/converters | ✅ `margin` + `spacing` |
| Frame delay control | GIF makers/converters | ✅ `delay_ms` with 10ms minimum and 60s cap |
| Loop control | GIF tools | ✅ `loop_count` (`0` = infinite) |
| Skip transparent/blank frames | Useful for padded sheets | ✅ `skip_empty` |
| Frame count cap | Useful for quick previews / huge sheets | ✅ `max_frames` |
| Pure browser-local / no ffmpeg | Some browser tools; better for gizza | ✅ pure Rust `image` GIF encoder, runs in chat/CLI surfaces |
| Visual animation preview page | Common in dedicated web apps | Not built: current gizza page renderer does not expose image-bytes/GIF output for this tool shape |
| Auto-detect frame bounds/grid | Some converters advertise it | Not built: explicit grid/tile parameters are deterministic and safer for CLI/chat |
| Reverse GIF-to-sprite-sheet | Ezgif/Morrow Shore adjacent feature | Out of scope; existing backlog/tool direction is sprite sheet → GIF |

## Notes

Although the backlog type hint was `ffmpeg`, this tool is implemented as pure Rust using `image::codecs::gif::GifEncoder`, which is preferable here because it runs in the chat block and CLI without the Service Worker ffmpeg limitation.

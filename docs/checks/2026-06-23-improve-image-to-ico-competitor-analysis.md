# image-to-ico — competitor analysis (2026-06-23)

Tool: `blocks/image-to-ico` — convert one source raster image (PNG/JPEG/WebP/GIF/BMP)
into a single multi-resolution Windows `.ico` favicon, returned as a downloadable file.

Surfaces: **chat + CLI** (pure Rust, runs on all backends including the chat Service
Worker). **No standalone page** — a binary `.ico` output has no page render mode (same
F3 pattern as `image-collage` / `favicon-generator`).

## Distinction from the existing `favicon-generator`

`favicon-generator` outputs a **ZIP bundle** (a multi-res `favicon.ico` with hard-coded
16/32/48 frames + nine square PNGs + `apple-touch-icon.png` + `site.webmanifest`).
`image-to-ico` outputs a **single `.ico` file** and lets the user choose exactly which
resolutions to embed in that ICO (favicon-generator's ICO frame set is fixed). Different
output shape, narrower use-case (drop a `favicon.ico` straight into a site root), and a
user-controlled ICO frame set — kept as a distinct tool, not skiplisted.

## Top competitors surveyed

1. **ConvertICO** (convertico.com) — PNG/JPG → ICO; multi-size selection 16–256;
   transparency preserved; "icon resizer" variant; drag-drop, no signup.
2. **favicon.io / favicon-converter** — PNG → favicon; emits a ZIP of standard sizes
   (16/32/48/180/192/512) + an `.ico`; bundle-oriented (closer to our favicon-generator).
3. **png2ico.com / Convertio png-ico** — single-purpose PNG→ICO; pick one or several
   embedded sizes; transparency support.
4. **favicon-generator.org** — full app-icon/favicon bundle generator (bundle output).
5. **pixoate ICO converter** — PNG/JPG/SVG → multi-size `.ico`; custom dimensions.

## Capability diff (competitor median vs. this tool)

| Capability                                   | Competitors | image-to-ico |
|----------------------------------------------|-------------|--------------|
| Multi-resolution single `.ico` output        | yes         | **yes** (user-chosen `sizes`, each 1..=256) |
| Standard size set as default                 | yes         | **yes** (16,32,48,64,128,256) |
| Custom / arbitrary sizes                     | some        | **yes** (any comma list; clamped to 256, deduped, sorted) |
| Transparency preserved                       | yes         | **yes** (RGBA throughout; default bg transparent) |
| Non-square source handling                   | varies      | **yes** (`fit` = contain pad / cover crop) |
| Background fill for padding                   | some        | **yes** (`background`, #rgb/#rrggbb/#rrggbbaa) |
| PNG/JPEG/WebP/GIF/BMP input                  | PNG/JPG     | **yes** (broader than the typical PNG/JPG-only tool) |
| No signup, instant, local                    | yes         | **yes** (runs client-side / in CLI) |

## Gaps considered

- **SVG input** — some converters (pixoate) accept SVG. The `image` crate cannot decode
  SVG (it's a vector format needing a rasterizer such as `resvg`/`usvg`). Out of scope for
  this tool; a dedicated SVG→raster step would be a separate concern. **Not built.**
- **Pixel-art / nearest-neighbour resampling** — a niche "pixel art optimization" toggle a
  couple of tools advertise. We use Lanczos3 (the standard high-quality filter); a
  nearest-neighbour mode is a minor future enhancement, not a median capability. **Not built.**
- **Bundle output (PNGs + manifest + apple-touch-icon)** — this is exactly what the
  existing `favicon-generator` tool already provides; intentionally NOT duplicated here so
  the two tools stay distinct (single `.ico` vs full bundle).

## Conclusion

For the single-`.ico` converter category, `image-to-ico` meets or exceeds the competitor
median: user-selectable multi-resolution frames, broad raster input, transparency, and
non-square fit control. The only competitor features omitted are out-of-model (SVG raster)
or already covered by a sibling tool (full favicon bundle). No in-model gap left open.

## Verification

- `cargo test --workspace` — 10 tests pass (9 core incl. ICO magic/frame-count/clamp/dedup
  + decode round-trip; 1 block drift-guard schema test).
- `wafer build` — chat `block.wasm` validates + instantiates (1325.9 KiB).
- CLI — `gizza tool image-to-ico url=<png> sizes=16,32,48` produced a valid type-1 ICO with
  3 frames at 16/32/48 px (verified by parsing the ICO directory).
- No page surface (binary `.ico` output) — stated, not claimed.

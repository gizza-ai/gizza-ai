# svg-to-png — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/svg-to-png` — rasterize an SVG document into a PNG or JPEG at a
chosen resolution. Pure-Rust (`resvg` + `usvg` + `tiny-skia`, `jpeg-encoder`).
SVG text input → image bytes output: **chat + CLI** (image-bytes output, like the
QR / chart / `latex-math-to-svg` tools, has no standalone page).

## What competitors do

- **CloudConvert — SVG to PNG** ([cloudconvert.com](https://cloudconvert.com/svg-to-png))
  — custom output size, DPI, and a render resolution per file; preserves
  transparency; import via link or Dropbox/OneDrive; bulk API. Powerful but the
  **SVG is uploaded to their servers** and the rich options sit behind an account
  for volume.
- **Convertio — SVG to PNG** ([convertio.co](https://convertio.co/svg-png/)) —
  clean drag-and-drop, transparency-preserving, pull from Google Drive/Dropbox or
  a URL; 100 MB/upload cap. Again **server-side upload**, ad-supported, limited
  free conversions.
- **svgtopng.com** ([svgtopng.com](https://svgtopng.com/)) — browser-based, claims
  local conversion; simple "drop SVG → download PNG". Minimal sizing controls
  (no explicit width/height/scale or JPEG/background options exposed).
- **Vexlio free SVG→PNG** ([vexlio.com](https://vexlio.com/svg-to-png/)) — quick
  one-off browser conversion at a fixed/derived size; PNG only.
- **Inkscape / `rsvg-convert` / a headless Chrome** — the local reference
  (`rsvg-convert -w 800 in.svg -o out.png`), exact and scriptable, but needs an
  install (librsvg / a browser) and isn't chat- or zero-setup runnable.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`resvg`/`usvg`/`tiny-skia`)
   compiled to wasm: the chat Service Worker and the CLI rasterize on-device. The
   SVG (often a logo / design asset) never leaves the machine — unlike
   CloudConvert/Convertio which upload it.
2. **Full sizing model in one call.** `width`/`height` in px (set **one** to keep
   the SVG's aspect ratio, or **both** to force an exact size); if neither is set,
   `scale` multiplies the SVG's intrinsic size (1.0 = native, 2.0 = @2x) — covering
   the "render at any resolution / DPI" feature the paid tools gate.
3. **PNG *and* JPEG.** `format=png` (lossless, keeps the alpha channel) or
   `format=jpeg` with a `quality` knob (1–100) for a smaller file — most free
   converters are PNG-only.
4. **Transparency *and* a background colour.** PNG keeps alpha; `background`
   ('transparent' or `#rgb`/`#rrggbb`/`#rrggbbaa`) fills behind the artwork, and
   JPEG (which has no alpha) composites onto it (white when transparent) so
   transparent regions don't turn black — a common gotcha in naive converters.
5. **Embedded raster images supported.** `<image href="data:image/png;base64,…">`
   (and JPEG/GIF/WebP data URIs) inside the SVG are rasterized into the output.
6. **Same everywhere.** Identical behaviour via chat and the CLI
   (`gizza tool svg-to-png svg=… format=… width=… scale=… background=…`).

## Honest scope / limitations

- **No standalone page.** Image-bytes output has no page render mode in gizza
  (same as the QR / chart / `latex-math-to-svg` tools); the surfaces are chat + CLI.
- **`<text>` needs a font.** To keep the wasm build filesystem-free (the wafer
  runtime provides no WASI fs imports — `path_open`/`fd_close`), the font database
  and disk/URL font + image loading are disabled. Shapes, paths, gradients,
  filters and embedded `data:` raster images render exactly; `<text>` without an
  embedded/converted font is skipped. **Convert text to paths in your editor** for
  pixel-perfect labels. (CloudConvert et al. run server-side with full system
  fonts, so they render text — the trade-off for local, private conversion.)
- **External file/URL `<image href>` references are not resolved** (no base dir /
  no network) — embed them as `data:` URIs instead.
- Output is capped at **8000 px per side** and 16 MiB to bound memory.

## Tests

11 core unit tests: rasterizes a shape SVG to a native-size PNG (verifies the PNG
magic + IHDR dimensions); `scale` multiplies the size; an explicit `width` keeps
the aspect ratio; explicit width+height force an exact size; JPEG output (SOI
magic); a `background` colour fills the PNG (decoded + pixel checked); errors on
empty / non-SVG input; rejects an over-`MAX_DIM` size; `OutputFormat::parse` and
`BgColor::parse` accept the valid forms and reject junk. Plus the block-level
chat-schema drift-guard test (authored JSON == `descriptor().to_schema_json()`).

**Build/verify:** `cargo test --workspace` (12 pass) · `wafer build` validates +
instantiates the chat block (1.5 MiB, fs-import-free) · `cargo install --path cli`
+ live CLI runs producing valid PNG/JPEG files at the requested sizes · generator
renders the 120-tool site with no page for this image-bytes tool.

## Sources

- [SVG to PNG — CloudConvert](https://cloudconvert.com/svg-to-png)
- [Convert SVG to PNG — Convertio](https://convertio.co/svg-png/)
- [svgtopng.com](https://svgtopng.com/)
- [Vexlio free SVG to PNG](https://vexlio.com/svg-to-png/)

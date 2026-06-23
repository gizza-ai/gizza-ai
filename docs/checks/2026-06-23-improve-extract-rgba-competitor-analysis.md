# extract-rgba — competitor analysis (2026-06-23)

## Tool summary
Decode an image (PNG/JPEG/WebP/GIF/BMP/TIFF/ICO) and export its raw per-pixel
RGBA8 values (0-255 r/g/b/a) as **text**, **CSV**, or **JSON**, in row-major
order, with an optional `max_pixels` cap for large images.

Surfaces: **chat** (pure-Rust wasm block, runs in the Service Worker) + **CLI**.
No standalone page — the page file-input path is ffmpeg-only, so this follows the
established no-page file-input pattern (same as `image-info`, `image-color-picker`,
`detect-file-type`).

## Top competitors surveyed
1. **onlinepngtools.com — "Convert PNG Pixels to a List"** — outputs each pixel's
   RGB/RGBA as a list; lets you pick the delimiter and include/exclude alpha.
2. **rapidtables.com / image color pickers** — single-pixel sampling, not a full dump.
3. **EZGIF "Pixel data" / "Image to array"** tools — dump pixels as a grid; some emit
   a JS/Python array.
4. **Python `Pillow` `img.getdata()` / `numpy.asarray(img)`** — the de-facto scripting
   baseline: a flat array of RGBA tuples, row-major.
5. **ImageMagick `convert img.png txt:-`** — emits `x,y: (r,g,b,a) #hex name` rows
   (the "TXT" pixel-enumeration format).

## Capability diff (what they do vs. extract-rgba)
| Capability | Competitors | extract-rgba | Decision |
|---|---|---|---|
| Per-pixel RGBA dump | yes | **yes** | core feature, covered |
| Row-major order | yes | **yes** | covered |
| Multiple output formats | some (list / array) | **text, csv, json** | covered (3 formats) |
| Coordinates (x,y) in output | ImageMagick txt:, EZGIF grid | **yes (CSV `x,y,...`)** | covered via CSV |
| Cap output size for big images | rarely | **yes (`max_pixels`)** | added — better than most |
| Wide input format support | PNG-only (onlinepngtools) | **PNG/JPEG/WebP/GIF/BMP/TIFF/ICO** | broader than PNG-only tools |
| URL or ref input (no upload) | upload only | **yes (url/ref)** | better fit for chat/agent use |
| Hex / named-color column | ImageMagick txt: | no | minor; rgb int channels are the scripting-friendly form. Out of scope for the "raw RGBA" framing — left to `image-color-picker` |
| Configurable delimiter | onlinepngtools | no | CSV + text already cover comma-delimited; extra delimiter knob is low-value |
| Grayscale/RGB-only output mode | some | no | RGBA is the lossless superset; callers drop the alpha column trivially |

## Gaps closed in this build
- **Three output formats** (text / csv / json) rather than a single fixed dump — matches
  the union of competitor output styles (list, spreadsheet, array).
- **`max_pixels` cap** so the tool degrades gracefully on large images instead of dumping
  millions of rows into a chat context, and reports `total_pixels` / `emitted_pixels` /
  `truncated` so the caller knows it was clamped.
- **Broad decoder support** (7 formats) vs. the common PNG-only web tools.
- **CSV carries `x,y` coordinates** (the ImageMagick `txt:` idea) while text/json stay compact.

## Out-of-model / not built (documented, not shipped)
- **Hex + nearest-color-name columns** (ImageMagick `txt:`): already provided per-pixel by
  the sibling `image-color-picker`; bundling it here would duplicate that tool. Kept the
  output to raw integer RGBA, which is the scripting baseline.
- **Configurable delimiter / array-literal (Python/JS) output**: low marginal value over
  text+csv+json; skipped to keep the schema small.
- **No standalone page**: image-file input has no page render mode in this stack (file-input
  is ffmpeg-only), so verification is chat + CLI only. Stated, not faked.

## Verification
- `cargo test --workspace`: 8 tests pass (7 core happy/error/truncation + 1 drift-guard schema test).
- `wafer build`: block.wasm validates and instantiates (pure Rust, runs in chat SW).
- CLI (`gizza tool extract-rgba`): verified text, csv, and json formats + `max_pixels`
  truncation against a live PNG.
- No page surface (by design); generator runs clean without it.

# color-channel-split — competitor analysis (2026-06-23)

Tool: `gizza-ai/color-channel-split` — extract a single colour channel from an
image as its own PNG, for steganography and colour analysis. Pure-Rust (`image`),
surfaces: **chat + CLI** (image-bytes output → no page, like `image-false-color` /
`colorblind-simulator` / `image-color-quantize`).

## What we ship

- **Channels:** red, green, blue, alpha (RGBA) + cyan, magenta, yellow, key (CMYK,
  derived from RGB via the standard naive conversion).
- **Render modes:** `grayscale` (default) — channel value as a gray level (R=G=B=v),
  the classic per-channel stego / forensics view; `color` — channel kept in its own
  colour slot, others zeroed (red→red-only, alpha→opacity over black, CMYK→ink colour).
- **Output:** PNG, same dimensions as the input. `url` or `ref` input.

## Competitors surveyed

| Tool | Channels | Modes | Notable |
|------|----------|-------|---------|
| [onlinetools.com – Separate Color Channels](https://onlinetools.com/image/separate-image-color-channels) | RGBA, CMYK, HSL | colored + grayscale | side-by-side compare; premium gates |
| [dCode – RGB Channels](https://www.dcode.fr/rgb-channels) | R/G/B | grayscale / "own color" | background colour (b/w), resolution presets |
| [onlinepngtools.com – Extract PNG Color Channels](https://onlinepngtools.com/extract-png-color-channels) | RGBA, CMYK, HSL | colored + grayscale | multi-channel at once, "make X grayscale" |
| [imageonline.io – Extract Color Channel](https://imageonline.io/extract-channel/) | R/G/B/A | grayscale + "pure channel in its color" | channel **combination** toggling (R+G→yellow), PNG/JPG/WebP export |
| [AAT Bioquest – Image Channel Splitter](https://www.aatbio.com/tools/online-image-channel-splitter-rgb) | RGB | colored | client-side, jpg/png/gif/bmp/tiff in |

## Gap diff + ranking (fit-to-model)

1. **CMYK separation (cyan/magenta/yellow/key)** — offered by 3 of 5 competitors.
   In-model (pure RGB→CMYK maths, no deps). **CLOSED:** added C/M/Y/K to the channel
   enum; grayscale + color (ink-colour) rendering, unit-tested against pure-red /
   black / white / cyan vectors.
2. **grayscale vs colored ("own color") render modes** — universal across competitors.
   **Already shipped** as `mode = grayscale | color`.
3. **Alpha channel extraction** — offered by onlinetools / onlinepngtools / imageonline.
   **Already shipped** (`channel = alpha`; grayscale → opacity-as-gray, color →
   opacity over black).

## Out-of-model / deliberately not built

- **HSL channel separation** (hue/saturation/lightness) — computable and a fair future
  add, but hue is a 0–360° angle that doesn't map cleanly to a 0–255 grayscale ramp the
  way RGBA/CMYK do, so it needs its own render contract; deferred to keep this tool's
  single-channel-image model clean. Not a blocking gap (only 2 competitors offer it).
- **Multi-channel / side-by-side output in one call** — our surface returns one image
  per call (image-bytes output has no page render-mode for a gallery); the chat/CLI
  caller invokes once per channel. Matches the existing image-bytes tool contract.
- **Channel *combination* (R+G→yellow recompose)** — that's a recombine operation, a
  different tool shape than a splitter; out of scope here.
- **JPG/WebP output formats** — we standardise on lossless PNG so channel/stego bit
  data survives (a JPEG re-encode would corrupt it); intentional.

No competitor copy, branding, or trademarks were used.

## Verification (this run)

- `cargo test` — 10 core unit tests (RGBA + CMYK vectors, modes, error paths) +
  1 drift-guard schema test, all pass.
- `wafer build` — chat `block.wasm` validates / instantiates (1373.9 KiB).
- `cargo install --path cli` + generator (216 tools) clean.
- CLI (`gizza tool color-channel-split`): red/green/alpha and cyan/magenta/key in both
  grayscale and color modes produce valid same-size PNGs; bad channel rejected with a
  clear message.
- No page surface (image-bytes output → CLI + chat only), stated per the no-page
  image-tool pattern.

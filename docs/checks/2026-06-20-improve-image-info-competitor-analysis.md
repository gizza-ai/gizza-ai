# image-info — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/image-info` — report an image's format, dimensions, color
type, and file size from its bytes. Chat + CLI (image input + text report; the
page file-input path is ffmpeg-only — the F3 no-page file-input pattern, like
detect-file-type / image-color-picker).

## What competitors do

- **Online "image info / EXIF / dimensions" sites** (metadata2go, getmetadata,
  imageonline) — upload an image, see properties. Strengths: often show EXIF.
  Weaknesses: the image is **uploaded to a server** (privacy), ads, and many
  only show dimensions or only EXIF.
- **`identify` (ImageMagick) / `file` / `exiftool`** — local and thorough, but
  each requires installing a tool and remembering flags.
- **OS "Get Info" / Properties** — interactive only, desktop-bound.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`image` crate) compiled to
   wasm: runs in the chat Service Worker and headless via the CLI. The image
   never leaves the device (the tool name promises exactly this).
2. **Decodes for ground truth.** Format is detected from the **magic bytes**, and
   dimensions/color type come from actually parsing the image — so a renamed or
   mislabeled file reports its real format, not its extension.
3. **Rich, structured output in one call** — format + MIME, width/height,
   megapixels, reduced aspect ratio (e.g. `16:9`), color type with bit depth
   (RGB/RGBA/grayscale), channel count, bits-per-pixel, alpha flag, and file size
   — all as JSON an agent or script can consume.
4. **Chainable.** Takes a `url` or a prior tool's `ref`, so you can inspect an
   image another tool produced.
5. **Honest errors.** Non-image / undecodable input errors clearly rather than
   guessing.

## Honest scope

- Reports decoded image properties, not **EXIF/metadata** (camera, GPS, ICC) —
  that would be a separate tool.
- Covers PNG/JPEG/WebP/GIF/BMP/TIFF/ICO (the `image` crate's common decoders).

## Tests

4 core unit tests (on images encoded in-test): a PNG RGBA reports format=PNG,
mime, 8×6 dims, 4 channels, 32 bpp, alpha true, color "RGBA (8-bit)", and a
reduced aspect ratio (8:6→4:3); a JPEG RGB reports 3 channels, no alpha, 16:9;
gcd reduction; and error cases (empty / non-image). Plus the block drift-guard
schema test. CLI verified over the wire on `tux.png` → PNG, 104×120, aspect
13:15, RGBA (8-bit), 32 bpp, alpha, 7666 bytes — dimensions/size consistent with
the PNG header and the file-hash/image-color-picker cross-checks.

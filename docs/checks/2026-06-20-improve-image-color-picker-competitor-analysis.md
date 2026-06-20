# image-color-picker — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/image-color-picker` — report the color at a pixel coordinate
in an image (hex, RGB(A), nearest name, dimensions). Chat + CLI (image input +
text report; the page file-input path is ffmpeg-only — the F3 no-page file-input
pattern, like detect-file-type).

## What competitors do

- **Online image color pickers / eyedroppers** (imagecolorpicker.com,
  pinetools, redketchup) — upload an image, click to read a color. Strengths:
  interactive click-to-pick. Weaknesses: the image is **uploaded to a server**
  (privacy), and they're click-only — not scriptable or callable by an agent.
- **OS eyedroppers / Photoshop** — great interactively, but desktop-only and not
  automatable.
- **ImageMagick** (`convert img.png -format '%[pixel:p{40,40}]' info:`) — local
  and scriptable, but requires installing ImageMagick and arcane syntax.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`image` crate) compiled to
   wasm: runs in the chat Service Worker and headless via the CLI. The image
   never leaves the device.
2. **Coordinate-addressable & scriptable.** Give `(x, y)` and get the color back
   as data — usable by an LLM/agent or a script, not just a manual click.
3. **Every representation at once.** hex (`#rrggbb`), hex with alpha
   (`#rrggbbaa`), `rgb(...)` and `rgba(...)` CSS strings, the raw r/g/b/a
   channels (0-255), and the image dimensions — no manual conversion.
4. **Friendly nearest-name.** Approximate nearest common color name (red, teal,
   orange, …) for a quick human label.
5. **Chainable.** Takes a `url` or a prior tool's `ref`, so you can sample a color
   from an image produced by another tool.
6. **Honest bounds.** Out-of-range coordinates error with the actual image size
   rather than returning garbage.

## Honest scope

- Reads one pixel (no region averaging / dominant-color palette yet — that would
  be a separate tool).
- The nearest-name palette is a compact ~19-color set (basic CSS colors), so the
  label is approximate, not the full X11/CSS4 name list.

## Tests

5 core unit tests (on PNGs built in-test): reads a solid color at a coordinate
with correct dimensions and hex/`#rrggbbaa`; reads a pixel with alpha
(`#10203080`); out-of-bounds coordinates error; undecodable bytes error; nearest
name for red/black/white/orange/near-black. Plus the block drift-guard schema
test. CLI verified over the wire on `tux.png` at (40,40) and **cross-validated
with ffmpeg**: the tool's rgb (144,143,142) and dimensions (104×120) match
ffmpeg's `crop=1:1:40:40` pixel and the PNG header exactly.

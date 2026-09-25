# bmp-converter — competitor analysis (2026-09-24)

Scan run **before** implementing, per the create-next-tool recipe. All competitor
observations below are **paraphrased**; no competitor copy, branding or trademarks are
reproduced or reused. Out-of-model items are *listed*, not built.

Search: "online BMP converter image to BMP bit depth 24-bit 8-bit 16-bit".

## Competitors skimmed (top 3 with real bit-depth control)

### C1 — online-converting.com/image/convert2bmp/
Reachable. The only scanned tool with a full colour-depth matrix.

| Surface | What it exposes |
| --- | --- |
| Colour depth | 32-bit (RGBA), 32-bit (RGB), 24-bit, 16-bit 5:5:5:1 RGBA, 16-bit 5:5:5:1 RGB, 16-bit 5:6:5 RGB, 8-bit indexed, 4-bit indexed, 1-bit indexed, 1-bit mono. Default 32-bit RGBA. |
| Row direction | Bottom→Top (default) or Top→Bottom |
| Quantization | Dropdown 0–8 (indexed depths only) |
| Dithering | Yes / No toggle |
| Resize | New pixel dimensions, new DPI, keep-aspect checkbox |
| Limits | 50 files per batch; no size limit published |

### C2 — convertft.com/image-tools/bmp-converter
Reachable but thin: it converts between JPEG/PNG/WebP/BMP/GIF/HEIC/HEIF/AVIF and
*describes* handling 8-bit palette/greyscale and 24/32-bit full-colour BMPs, but exposes
**no** user-facing depth, dither, or compression control — format choice only.
Server-side ("cloud"), files deleted after processing. 30 images/batch, 30 MB/file,
250 MP total, no signup.

### C3 — image.mantrapdf.com/image-to-bmp/
Reachable. Closest match to the backlog row's wording.

| Surface | What it exposes |
| --- | --- |
| Colour depth | 24-bit true colour (default), 8-bit (256), 4-bit (16), 1-bit mono |
| Compression | None (uncompressed) or RLE |
| Dithering | None / Floyd–Steinberg / Ordered (offered for ≤8-bit) |
| Background colour | Colour picker, used to flatten transparent sources |
| Inputs | JPG, PNG, GIF, SVG |
| Limits | "browser memory limits apply"; ~50 MB typical |
| FAQ topics | BMP vs PNG, why RLE, when to dither, size limits |

## Table stakes → where each one landed

| # | Table stake | Seen in | Verdict | Where it lives |
| --- | --- | --- | --- | --- |
| 1 | Selectable BMP bit depth | C1, C3 | **in-model** | `bit_depth` enum: `1`, `8`, `16-555`, `16-565`, `24`, `32` |
| 2 | 24-bit default | C3 | **in-model** | `bit_depth` default `24` |
| 3 | Both 16-bit layouts (5:5:5 and 5:6:5) | C1 | **in-model** | separate enum values `16-555` / `16-565` |
| 4 | Palette size / quantization knob for indexed output | C1 ("quantization"), C3 (implicit) | **in-model** | `colors` 2–256, default 256, slider |
| 5 | Dithering choice (not just on/off) | C3 (none/FS/ordered), C1 (yes/no) | **in-model** | `dither` enum `none`/`bayer`/`floyd_steinberg`/`sierra2_4a`, default `floyd_steinberg` |
| 6 | Background colour to flatten transparency | C3 | **in-model** | `background`, `kind = "color"`, default `#ffffff` |
| 7 | Greyscale output | C1 (1-bit mono / indexed), C2 (8-bit greyscale BMPs) | **in-model** | `grayscale` boolean, default false — with depth `8` it writes a true 8-bit greyscale BMP |
| 8 | Convert *from* BMP as well as to it | C2, backlog row wording | **in-model** | `format` enum `bmp`/`png`/`jpeg`, default `bmp` |
| 9 | Uncompressed output (BI_RGB) | C3 ("None") | **in-model** | always — the tool's headline. `16-565` uses BI_BITFIELDS, which is still uncompressed pixel data (documented on the page) |
| 10 | Wide input format support | C1, C2, C3 | **in-model** | any format ffmpeg decodes (PNG, JPEG, WebP, GIF, BMP, TIFF, …) |
| 11 | Stated size limit | C2, C3 | **in-model** | 8 MiB, stated on the page |
| 12 | 4-bit (16-colour) indexed BMP | C1, C3 | **out-of-model** | see below |
| 13 | RLE4 / RLE8 compression | C3 | **considered, rejected** | see below |
| 14 | Top-down row order | C1 | **out-of-model** | see below |
| 15 | Resize / DPI change during convert | C1 | **considered, rejected** | see below |
| 16 | Batch (30–50 files at once) | C1, C2, C3 | **out-of-model** | see below |

## Out-of-model / rejected — with the verified reason

- **4-bit indexed BMP (#12) — out-of-model.** Verified 2026-09-24 by a real spike:
  `ffmpeg -h encoder=bmp` lists pixel formats `bgra bgr24 rgb565le rgb555le rgb444le rgb8
  bgr8 rgb4_byte bgr4_byte gray pal8 monob`, and encoding with `-pix_fmt rgb4_byte`
  produced a file whose `biBitCount` header field reads **8**, not 4 — ffmpeg's BMP
  encoder maps every 4-bit-ish pixel format into the 8-bit branch and has no 4-bit
  writer. Writing 4-bit BMPs would need a hand-rolled encoder in a pure-Rust block,
  which has no page render mode for image bytes in this repo (see
  `references/page-patterns.md`), so it would cost the page surface to gain one depth.
  Depths 1, 8, 16, 24 and 32 all verified writing the correct `biBitCount`.
- **RLE4/RLE8 compression (#13) — considered, rejected.** The backlog row's scope is
  explicitly *uncompressed* BMP, and RLE would change the headline behaviour. ffmpeg's
  BMP encoder writes BI_RGB only in any case.
- **Top-down row order (#14) — out-of-model.** BMP signals top-down with a negative
  `biHeight`; ffmpeg's BMP encoder always writes a positive height (bottom-up). Verified
  in the same spike — every produced file had a positive height field.
- **Resize / DPI (#15) — considered, rejected.** Scope creep onto dedicated
  resize tooling; keeping a converter's output pixel-identical in size is the
  family invariant here.
- **Batch upload + ZIP download (#16) — out-of-model.** The page takes a single file and
  the chat runtime cannot run ffmpeg in a Service Worker; multi-input ffmpeg is recorded
  as un-buildable in `references/page-patterns.md`.
- **Server-side processing / accounts (C2) — out-of-model by design.** Everything here
  runs locally in the browser tab or the CLI; nothing is uploaded.

## UX control patterns adopted

- Depth as a labelled `<select>` (C1/C3 pattern) — implemented with `Param::enumv` plus
  `[input.labels]` so each value reads as e.g. "24-bit — true colour (16.7M)".
- Colour picker for the flatten background (C3 pattern) — implemented with the
  generator's `kind = "color"` hybrid swatch/text control.
- Palette-size control as a slider rather than C1's opaque 0–8 "quantization" dropdown,
  because an explicit colour count is self-explanatory.
- One-click presets, which none of the three ship — added as `[[example]]` chips
  (24-bit classic, 8-bit 256-colour, 1-bit mono, 16-bit high colour, back to PNG).
- FAQ topics mirror the *questions* C3 answers (BMP vs PNG, when to dither, size limits)
  with entirely original wording.

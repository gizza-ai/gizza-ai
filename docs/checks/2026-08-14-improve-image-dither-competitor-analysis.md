# image-dither — competitor analysis (2026-08-14)

Scan run **before** implementation, per `/create-next-tool` step 4. All notes are
paraphrased observations of publicly visible feature sets. **No competitor copy,
branding, or trademarks are reproduced here or in the shipped page.**

## Scope

Backlog row: *"Apply Floyd-Steinberg or ordered dithering to an image for retro,
pixel-art, or e-ink looks and palette reduction."* (type hint: `pure`).

## Duplicate check

`ls blocks/ | grep -iE 'dither|palette|posterize|pixel|quantiz|halftone|gif'` →
nearest neighbours inspected:

| Existing block | What it actually does | Overlap? |
| --- | --- | --- |
| `image-color-quantize` | NeuQuant palette reduction to N colours, **one** param (`colors`), no error diffusion anywhere in its core | No — flat quantization, zero dithering |
| `image-to-pixel-art` | Down-scales to a pixel grid then NeuQuant-quantizes; no dither modes | No |
| `image-pixelate-censor` | Mosaic redaction of a region | No |
| `video-to-gif` / `gif-optimize` | `palettegen`/`paletteuse` on **video → GIF**; dither is a fixed internal choice, not user-facing | No |

Verdict: **not a duplicate.** No block exposes a dithering algorithm, and none
offers a fixed/target palette. The backlog's own `image-posterize` row even
describes itself as *"not covered by the existing dither/pixel-art tools"*.

## Competitors reviewed

Three reachable, real tools (plus two more read from search result summaries
where the site itself is a client-rendered SPA with no server-side content):

1. **grayscaleimage.org — dithering effect generator.** Algorithms: Floyd-Steinberg,
   Atkinson, Ordered (Bayer), Stucki. Adjustable colour-level count spanning
   2-tone black & white through 16 levels. Two modes: monochrome and full-colour.
   Export PNG (explicitly recommended to preserve the pattern), plus JPG/WebP.
   Documents limits rather than sizes: JPEG compression damages dither patterns,
   resizing a dithered image blurs it, very small images make algorithms hard to
   tell apart, busy photos benefit from a contrast pass first.
2. **turbodither.com.** Algorithms: Floyd-Steinberg (default), Atkinson, Ordered
   (Bayer), Jarvis-Judice-Ninke. Large fixed-palette menu (retro console/computer
   palettes, grayscale sets, plus a user-defined custom palette). Numeric sliders
   for *diffusion factor* (default 1.00) and *dither scale* (default 0.50, described
   as pixel size of the dither pattern — larger = chunkier retro look), plus
   brightness / contrast / saturation sliders. Extras well outside a
   single-transform tool: live camera, ASCII filter, CRT/glitch effects, paint
   mode, steganography, audio-reactive mode. Stated limit: images over
   2000×2000 get slow; browser memory bounds very large images. Everything runs
   client-side; PNG export preserves transparency.
3. **ditheringstudio.com.** Image *and* video dithering, advertising
   Floyd-Steinberg, Bayer, Atkinson, Sierra and ~30 algorithms in total; exports
   PNG, JPG, WebP, SVG, GIF, MP4, WebM. Client-side, no upload.
4. *(summary only)* **ditherit.com** — Floyd-Steinberg, Atkinson, Bayer; a set of
   retro palettes; animated GIF and multi-image batch input; local processing.
5. *(summary only)* **ascii-magic.com dither style** — Floyd-Steinberg, Atkinson,
   ordered/Bayer plus halftone and line dithers across ~16 retro palettes.

## Table stakes → where each one landed

| Table stake | Decision | Where |
| --- | --- | --- |
| Floyd-Steinberg (the default everyone ships) | in-model | `algorithm=floyd_steinberg` (default) |
| Ordered / Bayer | in-model | `algorithm=bayer` |
| Atkinson | in-model | `algorithm=atkinson` |
| Sierra family | in-model | `algorithm=sierra2 / sierra3 / sierra2_4a` |
| Burkes | in-model | `algorithm=burkes` |
| Simple error diffusion (Heckbert) | in-model | `algorithm=heckbert` |
| No-dither comparison / plain quantize | in-model | `algorithm=none` |
| Bayer matrix coarseness | in-model | `bayer_scale` 0–5 (default 2) |
| Colour-level count (2 → 16 → 256) | in-model | `colors` 2–256 (default 16), used by `palette=auto` |
| Monochrome 1-bit black & white | in-model | `palette=mono` (exact `#000`/`#fff`, verified) |
| Grayscale level sets (e-ink) | in-model | `palette=gray4`, `palette=gray16` |
| Fixed retro palettes | in-model, **generically named** | `palette=green4` (4-shade green LCD), `amber2` (amber terminal), `cga4` (the 4-colour CGA standard). Deliberately descriptive names — console/computer brand names are trademarks and are not used. |
| User-defined custom palette | in-model | `palette=custom` + `palette_colors` (comma-separated hex, 2–16 entries) |
| Dither scale / chunky pixel size | in-model | `pixel_scale` 1–16 (nearest-neighbour down-then-up around the dither) |
| Contrast pass before dithering | in-model | `contrast` 0.5–3.0 (default 1.0) |
| PNG export, recommended default | in-model | `format=png` is our **default** (lossy formats smear the pattern) |
| JPEG / WebP / GIF export | in-model | `format=jpeg / webp / gif`, plus `same` |
| Transparency preserved | in-model | PNG/WebP/GIF path keeps the alpha channel via `paletteuse` alpha handling |
| Preset one-click looks | in-model | five `[[example]]` preset chips on the page |
| Local / no-upload processing | already true | the page runs ffmpeg in the browser; the CLI runs locally |

## Out-of-model (listed, not built)

Recorded so nothing is silently dropped:

- **Stucki and Jarvis-Judice-Ninke kernels.** `paletteuse` implements eight
  diffusion kernels; these two are not among them, and adding them would mean
  writing a second, pure-Rust dither path purely for kernel parity. The eight
  shipped kernels already cover the visual range (fine → chunky, sharp → soft).
- **Diffusion-factor slider** (partial error propagation). Not exposed by
  `paletteuse`; `bayer_scale` plus the kernel choice covers the same "how coarse"
  intent.
- **Brightness / saturation sliders.** Contrast is the one that materially changes
  a dither; the other two are a general image-adjust tool's job, and shipping a
  half-adjust panel here would blur this tool's scope.
- **Halftone / line / ASCII dithers.** Different rendering families (dot screens,
  glyph mapping), not palette dithering — `image-halftone` is its own backlog row.
- **Animated GIF and video dithering, batch/multi-image input, live camera.**
  The page's file input is single-upload and this block is an image transform;
  video dithering is a separate tool shape.
- **SVG export.** Vectorizing a dither pattern is a tracing problem, not a
  palette problem.
- **CRT / glitch / steganography / paint mode / audio-reactive.** Adjacent
  novelty features, out of scope for a single-purpose transform.

## Notes captured during the spike

- Fixed palettes are built entirely inside `-filter_complex` from `color=` sources
  `vstack`ed into the 16×16 (=256 pixel) image `paletteuse` requires — no second
  `-i` input, so it works unchanged on the browser ffmpeg build.
- A `scale=...:flags=neighbor` placed **after** `paletteuse` must be preceded by
  `format=rgb24`; without it the pal8→scale negotiation round-trips through
  subsampled chroma and a verified 4-colour output came back with 28 colours.
- Monochromatic palettes (`mono`, `gray4`, `gray16`, `green4`, `amber2`) get a
  `format=gray` stage first so the nearest-colour match follows luma rather than
  RGB distance.

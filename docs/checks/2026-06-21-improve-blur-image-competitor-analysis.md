# blur-image — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/blur-image` — apply a Gaussian blur of adjustable radius to
an entire image. Pure-Rust (`image`, `imageops::blur`). Image input → image
(PNG) output, so chat + CLI, no page (image-bytes output has no page mode — like
`sharpen-image` / `normalize-image` / `image-pixelate-censor`).

## Surfaces verified (Phase 1)

- **Chat block** — `wafer build` validated + instantiated `target/block.wasm`
  (1395 KiB, no missing WASI imports).
- **CLI** — `gizza tool blur-image url=<png> radius=4` returned a valid 200x200
  PNG (header + dimensions checked); `gizza list` shows the tool + description.
- **No page surface** — image-bytes output doesn't fit the page's text/field
  render model, consistent with the sibling image-editing tools. No Playwright
  spec applies.
- **Drift guard** — `schema_json_matches_authored_chat_schema` unit test passes
  (LLM-facing schema is single-sourced from the descriptor).

## What competitors do

- **Online "blur image" sites** — upload, drag a slider, download. Strength:
  zero-install. **Weakness: the image is uploaded** to a server; free tiers cap
  size/day and frequently recompress or watermark the result.
- **Photoshop / GIMP "Gaussian Blur"** — the reference, with a radius/sigma
  slider, but desktop apps and manual clicking; not scriptable in one call.
- **ImageMagick `-blur` / `-gaussian-blur`** — local, scriptable, excellent, but
  requires installing ImageMagick and learning its `radiusxsigma` syntax (and the
  `0xsigma` idiom most people get wrong).
- **CSS `filter: blur()` / canvas** — only blurs on screen; you can't get a
  blurred file out of it without extra plumbing.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`image`) compiled to wasm:
   runs in the chat Service Worker and headless in the CLI. The image never
   leaves the device.
2. **One intuitive knob with a real Gaussian.** `radius` is the Gaussian standard
   deviation in pixels (the value that actually controls the blur), with a sane
   default (5.0) and a clamp (0.1–200) so a typo can't hang on a giant kernel.
3. **Format-tolerant in, predictable out.** Accepts PNG/JPEG/WebP/GIF/BMP and
   returns a lossless **PNG**, so repeated edits don't accumulate JPEG artifacts.
4. **Chainable + agent-friendly.** Takes the image by `url` or `ref` and returns
   a downloadable PNG envelope (itself a `ref`), so it composes with the other
   image tools (resize → blur → border, etc.); identical from chat and CLI.

## Gaps considered (fit-to-model)

- **Selective / region blur** — already covered by the existing
  `image-pixelate-censor` tool (`mode=blur`, x/y/w/h). This tool is deliberately
  the whole-image case; no overlap added.
- **Motion / radial / lens blur** — out of scope for the `image` crate's box-pass
  Gaussian; would need custom kernels. Left out rather than half-built.
- **Preserve input format / metadata** — output is always PNG (lossless),
  consistent with the sibling tools; not changed.

## Honest scope

- **Whole-image Gaussian blur** (the standard method); not selective, motion, or
  lens blur.
- **PNG output** (lossless); it does not preserve the input's original format or
  metadata.
- **No page** — image input + image-bytes output don't fit the page's text/field
  model (consistent with the other image-editing tools).

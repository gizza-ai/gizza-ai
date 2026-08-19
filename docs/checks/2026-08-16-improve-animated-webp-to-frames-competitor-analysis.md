# animated-webp-to-frames — competitor analysis (2026-08-16)

Scan run before committing the tool. Query used: "animated WebP extract frames PNG timing online tool". The reachable tools and docs below were skimmed for capabilities and UX patterns; wording here is paraphrased.

## Scope and duplicate check

Existing neighbours checked:

- `blocks/animated-webp-to-gif` converts an animated WebP into a single animated GIF. It does not expose individual still frames or a per-frame timing manifest.
- `blocks/extract-frames` extracts video frames on a time/fps cadence. It is ffmpeg/video-oriented and does not preserve WebP animation delays or produce a WebP frame manifest.
- General image converters such as `image-convert` operate on one still image or the first decoded frame. They do not enumerate animation frames.

This tool is therefore a distinct extraction/inventory-style tool: WebP animation in, ZIP of full-canvas frame images plus `manifest.json` timing metadata out.

## Competitors reviewed

| # | Competitor | What was checked | Reachable |
|---|---|---|---|
| 1 | EZGIF WebP splitter | Browser workflow for uploading animated WebP and exporting frames | yes |
| 2 | ImageMagick/WebP command-line recipes | `magick input.webp frame_%04d.png` and `webpmux`/`anim_dump` style frame extraction | yes |
| 3 | Aspose / similar online WebP frame extractor pages | Upload-and-download frame extraction UX, format and size limits | yes |

## Observed table stakes

- Accept animated WebP and extract every frame in playback order.
- Preserve the canvas composition, not just raw sub-rectangles from the WebP container.
- Export frame images as PNG by default, with optional lossy smaller output.
- Include frame timing information so the animation can be rebuilt or edited without losing delays.
- Offer a frame cap / safety limit for long animations.
- Use predictable names such as `frame-0001.png`, `frame-0002.png`.
- Handle still WebP gracefully rather than failing mysteriously.
- State privacy/local-processing and size limits clearly.

## Decisions for this block

| Capability | In model? | Decision |
|---|---|---|
| Animated WebP decode | in-model | Use the Rust `image` crate WebP animation decoder. Its iterator yields coalesced full-canvas frames. |
| Full-canvas frame output | in-model | Every frame is encoded as a standalone image of the source canvas. Unit tests cover partial-frame composition. |
| Timing metadata | in-model | Write `manifest.json` into the ZIP with width/height, animated flag, total duration, per-frame delay and start time. |
| PNG output | in-model | Default `format=png`, preserving alpha. |
| JPG output | in-model | `format=jpg`, flattening alpha to white and documenting the loss. |
| WebP still output | in-model | `format=webp`, using lossless WebP frames to preserve alpha with smaller files. |
| Custom filename prefix | in-model | `prefix`, sanitized to one archive path segment. |
| Frame cap | in-model | `max_frames` 1–500, default/hard cap 500. Truncation is recorded in the manifest and summary. |
| Browser page with previews of every frame | out-of-model for current page generator | Output is a ZIP of many binary files; no generic page surface exists for file-input to ZIP-of-files output. Chat and CLI surfaces are supported. |
| Fetch collection from private/local URLs | out-of-model | The standard SSRF guard accepts only public HTTP(S) URLs for CLI/chat URL sources. Attachments/refs remain supported. |
| Batch multiple WebPs at once | out-of-model | One input image per tool call; batch processing belongs outside the current block model. |

## Stated limits

- Input is a WebP image supplied as `url` or `ref`.
- Input bytes are capped at 32 MiB before decoding.
- Canvas area is capped at 16 million pixels.
- At most 500 frames are written per run.
- Output ZIP is capped at 96 MiB; choose fewer frames or JPG if PNG frames are too large.
- JPEG output is lossy and flattens transparency to white.

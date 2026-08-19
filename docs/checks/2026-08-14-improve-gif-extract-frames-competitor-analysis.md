# gif-extract-frames competitor analysis — 2026-08-14

## Sources reviewed

Search query: `online GIF frame extractor split animated GIF into frames tool`.

Representative tools from the result set:
- Ezgif Split GIF image into frames
- Online GIF Tools Extract GIF Frames
- ImageToStl GIF Frame Extractor
- GIFGIFs Split GIF

This note paraphrases observed behavior and uses it only to define table-stakes behavior for the gizza `gif-extract-frames` block.

## Table-stakes capabilities

| Capability | Common competitor behavior | In model? | Decision for `gif-extract-frames` |
| --- | --- | --- | --- |
| Animated GIF input | Upload or provide an animated GIF and split it into its frames. | Yes | Accept an image source (`url` or `ref`) and decode GIF bytes directly. |
| Individual frame outputs | Produce one image per animation frame. | Yes | Write `prefix-0001.png`, `prefix-0002.png`, etc. into a ZIP. |
| Coalescing / unoptimize | Many splitters have an unoptimize/coalesce option so optimized partial frames become full canvas images. | Yes | Use the decoder's composited frame iterator and document that output frames are full-canvas PNGs. |
| Download archive | Competitors commonly offer a batch download. | Yes | Return a single `application/zip` payload. |
| Output format choices | Some tools allow PNG, GIF or JPG frames. | Partly | PNG is implemented because it preserves alpha and exact pixels; other formats are out of scope for this pass. |
| Frame delays/order | Some tools display timing and frame order. | Yes | Include `manifest.json` with frame index, filename, delay, dimensions and total duration. |
| Cap or frame limit | Large animations can produce huge output. | Yes | Expose `max_frames` with a 1–500 range and record truncation in the manifest. |
| Filename customization | Some tools let users control output naming. | Yes | Expose `prefix` for frame filenames. |
| Frame gallery/editor | Rich web tools preview, delete or reorder frames. | No | Out of model for this no-page file-to-ZIP tool. |
| APNG/WebP extraction | Some splitters handle APNG/WebP too. | No | Out of scope; this block is specifically GIF. Existing/converter tools can cover other formats separately. |

## UX / surface implications

The natural gizza surface is chat/CLI file-to-ZIP output rather than a generated text page. The result includes an LLM-readable summary plus a UI ZIP envelope; no sliders or preset chips apply. The two controls that do apply are `prefix` and `max_frames`, both included in the descriptor.

## Fit notes

A pure-Rust implementation fits the gizza model better than an ffmpeg-only block: `image` decodes animated GIF frames and `zip` packages PNG outputs without native binaries. The implementation bounds canvas pixels, input bytes and frame count so large GIFs fail with actionable errors rather than exhausting the wasm sandbox.

# Competitor analysis — images-to-animated-webp

Date: 2026-08-20
Tool: `images-to-animated-webp`
Backlog prompt: combine a set of images into a single animated WebP, far smaller than an equivalent GIF.

## Competitors scanned

Search: "combine images into animated WebP online frame delay loop count resize fit background color".

1. ImageMatrix animated WebP maker
   - Table stakes observed: multiple image frames, frame delay, loop count, compact animated WebP output.
   - UX pattern: upload/list of frames plus simple timing controls.
2. Scanly animated WebP maker
   - Table stakes observed: multiple images, frame duration, loop count, quality/size oriented output, browser-local processing claims.
   - UX pattern: straightforward frame upload and duration/quality controls.
3. Pixes animated WebP maker
   - Table stakes observed: JPG/PNG frame input, frame rate/duration, loop count, in-browser generation.
   - UX pattern: upload frames, set timing, export result.
4. webpmux reference docs
   - Table stakes observed: explicit frame list, per-frame duration, loop count, background color, animated WebP container assembly.
   - UX/CLI pattern: repeated frame options plus global loop/background options.

## Feature decisions

| Capability | In model? | Decision |
|---|---:|---|
| Ordered image list | Yes | `images` is a required `source_list`, one source per frame. |
| Uniform frame delay | Yes | `delay_ms` integer, 10–60000 ms, default 200. |
| Per-frame delays | Yes | `frame_delays_ms` comma/space/semicolon-separated list, one delay per source image. |
| Loop count | Yes | `loop_count`, 0 = forever, 1–65535 = finite plays. |
| Playback order presets | Yes | `order=forward|reverse|boomerang`; boomerang avoids repeated endpoints. |
| Resize/downscale | Yes | `max_width` scales the shared canvas down without upscaling. |
| Fit/crop behavior | Yes | `fit=contain|cover|stretch`. |
| Background / transparent padding | Yes | `background` accepts `#rgb`, `#rrggbb`, `#rrggbbaa`, `transparent`. |
| Size reduction / palette tuning | Yes | `colors=2..256` quantizes frames before lossless WebP; `0` preserves full color. |
| Lossy quality slider | No | The available wasm-safe Rust path encodes lossless VP8L frames. A lossy-quality slider would require a different animated WebP encoder path; left out rather than faking control. |
| Drag-reorder browser upload UI | No standalone page | `source_list` multi-file inputs are chat/CLI surfaces in this repo; no generated page is shipped for this tool. |

## Implementation notes

The tool uses pure Rust rather than ffmpeg/libwebp bindings: `image` decodes frames, `image-webp` encodes each frame as VP8L, and the block assembles the extended animated WebP RIFF container (`VP8X`, `ANIM`, `ANMF`) directly. Each frame is a full-canvas independent keyframe with blending disabled, avoiding ghosting/frame-disposal surprises.

Limits are explicit: 300 source frames, 4 MP canvas, 16,383 px WebP dimension cap, 8 MiB per resolved source, and 48 MiB encoded frame payload cap before the 64 MiB media envelope cap.

## Verification coverage

- Core unit tests cover multi-frame output, single-frame output, alpha/transparent padding, order and per-frame delays, fit/max-width, palette quantization, loop count, parser helpers, and error cases.
- Descriptor drift guard checks the live schema against an authored schema.
- CLI verification exercised `order=reverse`, `fit=cover`, short hex `#f00`, `colors=8`, `loop_count=1`, network source fetches, and a graceful fetch-error case.
- Chromium WebCodecs decoded the CLI result as two animated WebP frames at 64×48 with 120 ms duration each.

## Out-of-model / intentionally not shipped

- A browser page with drag-reorder uploads and a visual frame timeline is not part of the current generated page model for `source_list` plus binary image output tools.
- Lossy WebP quality control is deferred until the repo has a wasm-safe animated WebP encoder path that exposes lossy frame quality.

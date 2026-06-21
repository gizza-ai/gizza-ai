# animated-webp-to-gif — competitor analysis (2026-06-21)

Tool: `gizza-ai/animated-webp-to-gif` — converts an animated (or still) WebP into an
animated GIF for maximum compatibility. Pure-Rust (`image` crate: WebP animation decoder
+ GIF encoder), so it runs on every backend including the chat Service Worker — no ffmpeg.

Surfaces verified:
- **Chat skill / LLM API:** `wafer build` instantiates the block.wasm (815.9 KiB); drift-guard
  schema test passes (`schema_json_matches_authored_chat_schema`).
- **CLI:** `gizza tool animated-webp-to-gif url=… [loop=…] [speed=…] [delay=…] [width=…]` —
  produced a valid `GIF89a` with 12 frames + NETSCAPE2.0 infinite-loop block; `loop=once`
  drops the loop block; `width=200` resizes a 400×400 source to 200×200 (aspect preserved).
- **Page:** none. Image-bytes input + image-bytes output has no page render mode in the
  generator (same as `flip-image`), so this tool is chat + CLI only.

## Competitor feature scan (top 5 WebP→GIF converters)

| Tool | Notable WebP→GIF options |
| --- | --- |
| ezgif.com | encoder choice (gifski/libvips), preserves speed/frames/size, 256-color, 1-bit transparency, 200 MB cap; advanced edits via separate GIF Maker |
| CloudConvert | resolution/dimensions, frame rate (fps), quality, target file size, batch, cloud/URL imports |
| Convertio | resolution, quality, file-size controls, preserves all frames/timing, 1 GB cap, cloud/URL imports |
| FreeConvert | resize, compression slider (1–200, default 75), auto-orient (EXIF), strip metadata, alignment, presets, "apply to all", 1 GB cap |
| Aspose | batch (≤10 files), output-format dropdown, delivery (download/email/Dropbox) |

### Union of distinct capabilities
fps / frame-rate control · animation timing & frame preservation · resize/dimension control ·
quality/compression control · target file-size control · 256-color (GIF fixed) · transparency /
encoder choice · auto-orient / strip metadata / alignment · presets / apply-to-all · batch
conversion · large file-size caps · cloud/URL imports · delivery options (download/email/cloud) ·
frame reorder / speed / merge (ezgif GIF Maker).

## Gap assessment & actions

**Closed (in-model, pure-Rust):**
- **Frame + timing preservation** — every WebP frame and its per-frame delay are carried into
  the GIF (zero-delay frames default to 100 ms). Matches ezgif/Convertio's core promise.
- **Quality control** — `speed` (1–30) exposes the GIF encoder's palette-quality/speed knob;
  lower = better palette. Covers the quality/compression axis competitors expose.
- **Frame-rate / timing control** — `delay` overrides every frame's delay in ms (equivalent to
  forcing a uniform fps), useful for fixing broken/zero timings. Covers CloudConvert's fps knob.
- **Resize / dimensions** — added `width` (px); height scales to preserve aspect ratio. Covers
  the resize axis present in CloudConvert/Convertio/FreeConvert. Verified via CLI (400→200).
- **Loop control** — `loop` = `infinite` (default) or `once`. Notably, *none* of the five
  competitors expose explicit GIF loop on/off on their dedicated pages — this is a differentiator.

**Out of model / intentionally not built (no in-model surface):**
- **Batch / multi-file conversion** — the chat + CLI surfaces take one source per call; multi-file
  upload isn't a supported page/SW shape here (same constraint as other media tools). Out of scope.
- **Cloud imports (Drive/Dropbox/OneDrive), email delivery, 24h auto-delete, file-size caps up to
  1 GB** — these are SaaS hosting/storage features, not conversion logic. The tool already accepts
  any public `url` or a prior-tool `ref`; input is bounded to 16 MiB / output 48 MiB by design.
- **Auto-orient (EXIF) / strip-metadata / alignment** — EXIF orientation is a JPEG/TIFF concern;
  WebP frames carry no such tag, and GIF output stores no metadata, so these are no-ops here.
- **Encoder choice / advanced dithering** — the `image` crate exposes a single GIF encoder with a
  speed/quality knob (already surfaced as `speed`); a second encoder backend isn't available
  in-model. GIF is inherently 256-color; the encoder handles palette quantization.

No competitor copy, branding, or trademarks were used. Out-of-model features are listed, not built.

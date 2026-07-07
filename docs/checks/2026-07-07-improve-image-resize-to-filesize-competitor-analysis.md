# Competitor analysis — image-resize-to-filesize (2026-07-07)

Tool goal: take an image + a target file size in **KB** and iteratively re-encode
(binary-search the encoder quality) — optionally shrinking width first — so the output
lands at or under the target. Distinct value vs the existing `image-compress` block:
`image-compress` takes a *fixed* quality knob; this tool *searches* for the quality that
hits a byte budget. (`image-compress` and `image-shrink-for-sharing` both note in their
own source that true target-size needs an iterative search they don't do.)

All findings paraphrased — no competitor copy, branding, or trademarks reproduced.

## Competitors scanned

1. **VSPIC — "compress image to target KB"** (vspic.com). Single numeric target in KB;
   quality preview before download; one image per run. Accepts JPG/PNG/WebP/GIF/HEIC/AVIF/
   BMP/TIFF/SVG/ICO; outputs JPG or WebP toward the KB goal. Method: dynamic re-encode
   (quality), not dimension resizing. Stated limit: very detailed photos may not reach an
   aggressive KB target without visible quality loss. Fully browser-local.
2. **MB2kB** (mb2kb.com). Numeric target + a **KB/MB unit toggle**. Preset buttons: 20/50/
   100/150/200/250/300/350 kB. No quality slider, no format selector, no resize control.
   Inputs JPEG/PNG/WebP; keeps input format. Lossy JPEG recompression ("drops detail the
   eye won't notice"). Practical input cap ~10-15 MB. Browser-local, single image on web.
3. **Watermarkly — JPEG compressor, "Specific File Size" mode** (watermarkly.com). Three
   modes: Better Quality / Smaller Size / Specific File Size (target in KB or MB). Presets
   referenced at 20/50/100/200 KB. JPEG in/out, batch + rename. Doesn't state whether the
   target is met by quality alone or by resizing.

(iloveimgy "resize image in KB" returned HTTP 403 and was replaced by Watermarkly.)

## Table-stakes → decision

| Capability / UX | In competitors | Our decision | In model? |
|---|---|---|---|
| Numeric target size in **KB** | all 3 | `target_kb` param, required, min 1 | in-model ✓ |
| Preset target buttons (50/100/200 KB…) | MB2kB, Watermarkly | `[[example]]` preset chips (50/100/200 KB jpg, 100 KB webp) | in-model ✓ |
| Output format JPG/WebP | VSPIC | `format` enum `jpg`\|`webp`, default `jpg` | in-model ✓ |
| Auto quality search to the byte budget | all 3 (hidden) | binary quality-search 5..95 to highest fit ≤ target | in-model ✓ (the core value) |
| Optional resize / max width | (VSPIC/MB2kB hide it) | `max_width` param (0 = keep; shrink-only cap) so an aggressive target unreachable by quality alone can still be met | in-model ✓ |
| Show achieved size + quality used | VSPIC (preview) | status line reports final KB + quality picked | in-model ✓ |
| Private / browser-local | all 3 | wasm + ffmpeg.wasm on the page; native ffmpeg in CLI | in-model ✓ |
| KB/MB unit toggle | MB2kB | Considered, minor: kept **KB-only** (the tool's spec is KB; 1 MB = 1000 KB, stated in copy) | considered, rejected |
| Batch / multi-image | MB2kB (apps), Watermarkly | single input, no accounts | out-of-model |
| HEIC/AVIF/TIFF/SVG/ICO input | VSPIC | ffmpeg.wasm decodes common raster (jpg/png/webp/gif/bmp); exotic inputs error gracefully and are stated as a limit | out-of-model (partial) |
| PNG target-size | VSPIC (as input) | PNG has no lossy quality knob → excluded from the search; output is jpg/webp (stated on page) | considered, rejected |

## Honesty notes

- If even the lowest quality (and the chosen `max_width`) still exceeds the target, the tool
  returns the **smallest** result it produced and says so, suggesting a smaller `max_width` —
  same honest limitation VSPIC states for very detailed photos.
- Chat surface: ffmpeg can't run inside the chat Service Worker, so this tool is supported on
  its **standalone page** and the **CLI** (the page drives the search loop via `ffmpegExec`,
  the CLI/chat block drives it via `dispatch_ffmpeg`). Stated on the page.

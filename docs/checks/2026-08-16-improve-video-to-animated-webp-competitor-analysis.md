# video-to-animated-webp — competitor analysis (2026-08-16)

Scan run before implementation. Query used: "video to animated WebP converter online ffmpeg quality fps width". The notes below are paraphrased.

## Duplicate check

- `blocks/video-to-gif` converts a video section to GIF with a palette workflow. This tool targets animated WebP instead, which is a different output format with different quality/alpha/size trade-offs.
- `blocks/animated-webp-to-gif` converts animated WebP to GIF, the opposite direction.
- `blocks/animated-webp-to-frames` extracts frames from an animated WebP; it does not encode video into animated WebP.

So this is not a semantic duplicate.

## Competitors reviewed

| # | Competitor | Observed capabilities | Reachable |
|---|---|---|---|
| 1 | EZGIF video-to-WebP | Upload video, set start/end, size, fps, lossy/lossless/quality, output animated WebP | yes |
| 2 | CloudConvert video-to-WebP | Video file conversion, format presets/options, cloud workflow | yes |
| 3 | ffmpeg/libwebp recipes | `-c:v libwebp`, `-loop 0`, fps/scale filters, quality/lossless controls | yes |

## Table stakes and decisions

| Capability | In model? | Decision |
|---|---|---|
| Video input | in-model | Use `Input::Video`, `AssetKind::Video`, ffmpeg-runtime. |
| Start/duration trim | in-model | `start` and `duration` params. |
| FPS control | in-model | `fps` 0–60, default 12. |
| Resize by width | in-model | `width` 0–4096, height auto/even. |
| Lossy quality | in-model | `quality` 0–100, default 80. |
| Lossless WebP | in-model | `lossless` checkbox, skips quality. |
| Loop forever | in-model | Always `-loop 0`, stated in docs. |
| Preserve transparency | in-model when source/codec supports it | WebP/libwebp supports alpha; lossy and lossless modes both keep alpha. |
| Audio | out-of-model | Animated WebP is image-only; audio is dropped. |
| Batch conversion | out-of-model | One video per run. |
| Cloud URL import/private storage | out-of-model | CLI/chat URL fetch uses public HTTP(S) only; page uses local upload. |

## Stated limits

- Input and output are capped at 25 MiB.
- Short clips are recommended; long high-fps animations can still be large.
- Older apps that do not animate WebP may need GIF instead.

# Competitor analysis: video-to-dnxhd

Date: 2026-08-16
Tool: `video-to-dnxhd` — transcode video into an Avid DNxHD/DNxHR-style editing intermediate.

## Competitive scan summary

Reviewed common DNxHD/DNxHR conversion workflows: browser video converters that expose DNxHD as an output codec, desktop transcoder presets for DNxHD/DNxHR MOV/MXF, and ffmpeg recipe-style DNxHR documentation/examples. The useful table stakes are less about branded UI copy and more about exposing the professional interchange knobs without making invalid ffmpeg combinations easy to choose.

## Table stakes and fit decisions

| Capability / UX pattern | In model? | Decision in this tool |
| --- | --- | --- |
| Upload a local video and return a downloadable converted file | Yes | Page uses a `video/*` file input and ffmpeg runtime; chat/CLI schema accepts `url` or `ref`. |
| Choose DNxHD/DNxHR quality tier / profile | Yes | Descriptor exposes `dnxhr_lb`, `dnxhr_sq`, `dnxhr_hq`, `dnxhr_hqx`, and `dnxhr_444` as an enum with labels and preset chips. |
| MOV or MXF wrapper selection | Yes | `container=mov|mxf`; output extension and MIME follow the container. MXF gets ffmpeg's `-strict unofficial` relaxation for non-broadcast rates. |
| Optional resolution downscale | Yes | `resolution=source|2160p|1080p|720p`; filter caps height without upscaling. |
| Pixel format control | Yes, with validation | Exposes `auto|yuv422p|yuv422p10le|yuv444p10le`, but core validates against the selected profile because ffmpeg rejects invalid DNxHR/profile pairings. |
| Audio handling | Yes | `pcm16|pcm24|copy|none` covers NLE-friendly PCM, source-copy when safe, and picture-only files. |
| Classic fixed-bitrate DNxHD modes | Out of model for reliable arbitrary uploads | Not exposed. ffmpeg's fixed DNxHD modes require exact raster/frame-rate/bitrate table matches and fail for common phone/screen-capture sources. DNxHR is resolution-independent and fits this tool model. |
| OP-Atom MXF per-track Avid media | Out of model | Not built. ffmpeg `mxf_opatom` is single-stream; the page/CLI tool returns one file with video+audio, so OP1a is the viable wrapper. |
| Cloud batch queues / account storage | Out of model | This repo's tool pages run locally and chat/CLI return one envelope; no account, queue, or hosted storage. |

## Defaults

- Profile: `dnxhr_sq`, the general standard-quality editing tier.
- Container: `mov`, because it is the broadest desktop wrapper.
- Resolution: `source`, so the tool does not alter framing unless asked.
- Pixel format: `auto`, because DNxHR profiles dictate the only valid format.
- Audio: `pcm16`, the safest uncompressed NLE-friendly choice.

## Worked examples to cover

1. General edit intermediate: `dnxhr_sq`, `mov`, `source`, `auto`, `pcm16`.
2. Offline/proxy hand-off: `dnxhr_lb`, `mxf`, `720p`, `auto`, `pcm16`.
3. 10-bit grading hand-off: `dnxhr_hqx`, `mov`, `source`, `auto`, `none`.

## Verification implications

Browser playback is not a reliable correctness check for DNxHR/MXF. Tests should assert downloadable data URLs and inspect bytes for container/codec markers (`ftypqt`/`AVdh` for MOV; MXF header bytes and DNx-related ASCII markers where available). CLI tests should also exercise invalid pixel-format/profile combinations so users get actionable validation errors before ffmpeg runs.

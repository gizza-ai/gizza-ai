# Competitor analysis: video-deinterlace (2026-08-15)

## Scope

Tool: `video-deinterlace` — remove interlacing combing artifacts from uploaded video and output progressive frames. The gizza model fit is an ffmpeg-backed browser/CLI tool: local file in, local media out, no server upload, no ML model.

## Sources reviewed

- FFmpeg Micro Blog guide for yadif/bwdif deinterlacing: documented yadif and bwdif, mode choices, quality/speed tradeoffs, field order and worked commands.
- ElysiaTools online Video Deinterlace: browser-style upload flow, advertised support for YADIF/BWDIF/W3FDIF, and simple output-download UX.
- MPEGFlow deinterlace recipe: command-focused workflow highlighting yadif/bwdif/nnedi choices, field order, and when inverse telecine is the right operation instead.
- FFmpeg filter documentation for `bwdif`: authoritative option names for `mode`, `parity`, and `deint`.

## Table stakes and decisions

| Capability / UX pattern | Competitor signal | In gizza model? | Decision |
| --- | --- | --- | --- |
| Upload a local video and download a processed video | Online tools expose file upload/download | Yes | Page uses `video/*` input and `format = "video"`; CLI accepts URL/ref video source. |
| YADIF deinterlacer | Common ffmpeg examples and docs | Yes | Exposed as `filter = yadif`. |
| BWDIF deinterlacer | Guides recommend it as sharper/newer than yadif | Yes | Default `filter = bwdif`. |
| W3FDIF / NNEDI-style options | Some tools mention additional filters | Partly / no | Not exposed initially: W3FDIF has a different option shape and NNEDI is not in the local wasm ffmpeg build. Listed as out-of-model for this tool revision rather than hidden behind a broken enum. |
| Keep frame rate vs one frame per field | ffmpeg mode controls `send_frame` vs `send_field` | Yes | `mode = frame|field`; page labels explain 50i→25p vs 50i→50p. |
| Field order override | Guides call out top-field-first / bottom-field-first for jitter | Yes | `field_order = auto|tff|bff`. |
| Deinterlace all frames vs only flagged frames | ffmpeg has `deint=all|interlaced` | Yes | `apply_to = all|flagged`; default all because captures often lack reliable flags. |
| Inverse telecine for film-sourced 29.97i | Command recipes separate it from deinterlacing | No for this tool | Explicitly documented as a limit; a separate IVTC tool should use `fieldmatch,decimate`. |
| Preset chips | Online tools favor simple presets | Yes | Page includes camcorder/DV, broadcast 1080i double-rate, and mixed-footage presets. |

## Defaults

- `filter = bwdif`: best general-purpose detail preservation among the fast ffmpeg filters available here.
- `mode = frame`: keeps the nominal frame rate and avoids surprising file-size/runtime increases.
- `field_order = auto`: most containers/codecs carry usable flags.
- `apply_to = all`: safer for re-encoded or captured files whose interlacing flags are missing.

## Worked examples to cover

1. Camcorder/DV bottom-field-first capture: `bwdif`, `frame`, `bff`, `all`.
2. 1080i broadcast to smoother progressive motion: `bwdif`, `field`, `tff`, `all`.
3. Mixed progressive/interlaced material: `yadif`, `frame`, `auto`, `flagged`.

## Out-of-model / deferred

- ML/deep deinterlacers and NNEDI-style neural interpolation are outside the current pure ffmpeg/browser runtime.
- Batch/multi-file processing is outside the single-file page contract.
- Inverse telecine is deliberately separate because it drops duplicate telecine frames and can damage true interlaced video.

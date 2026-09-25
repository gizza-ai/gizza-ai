# video-bitrate-checker competitor analysis (2026-09-23)

## Scan

Query: `online video bitrate checker per stream bitrate tool`

Reviewed tools and adjacent table-stakes:

| Tool | What it offers | Table-stakes found | Model fit decision |
| --- | --- | --- | --- |
| SmoothCapture video bitrate checker | Browser-style upload flow for MP4/MOV/WebM with total bitrate, resolution, frame rate and duration. Emphasizes local/private processing. | File upload, total bitrate, duration, resolution, frame rate, privacy note, readable summary. | In model: local file page, total bitrate, duration, resolution, frame rate, no-upload copy. Added. |
| AdValify video bit rate detector | Upload-based ad validation; checks video bitrate in kbps against technical specs and advertises API use. | kbps output, validation/pass-fail against external spec, ad-delivery wording, upload/file source. | In model: kbps, user min/max thresholds, PASS/FAIL, ad-spec preset. Out of model: hosted API/VAST compliance workflow. |
| BitrateCalc / streaming bitrate calculators | Not a detector, but users expect platform-style presets and Mbps guidance by resolution/framerate. | Mbps units, 1080p/4K presets, delivery caps, simple form controls. | In model: Mbps unit, preset chips for 1080p/4K/video/audio/web cap. Out of model: recommended bitrate calculation from resolution/fps rather than measuring a real file. |

## Capability decisions

Implemented in-model:

- Overall bitrate: file size × 8 ÷ container duration.
- Per-stream bitrate: measured by summing demuxed packet payload bytes per track and dividing by that track's duration.
- Range checks: `min_bitrate`, `max_bitrate`, `units=kbps|Mbps`, `target=overall|video|audio`.
- PASS / FAIL / INFO status with reason and summary.
- MP4/MOV/M4A and Matroska/WebM stream metadata, including kind, codec, dimensions, frame rate, audio sample rate and channels when present.
- Browser page file upload with local-only/no-upload copy.
- Preset chips for report-only, 1080p30 video, 4K30 video, web delivery cap, ad spec cap and audio minimum.
- Worked examples and FAQ covering overall-vs-stream overhead, ffprobe parity, thresholds, audio-only target, privacy and limits.

Out of model / intentionally not built:

- Hosted API or VAST ad-tag compliance service.
- Live recommendations from changing platform policies.
- Peak/VBV analysis; this tool reports average bitrate over the file/track duration.
- Full video decode or visual quality analysis. The tool only demuxes packet payloads.

## UX/control notes

- File control accepts `video/*,audio/*` because the same measurement is useful for audio-only files and the block can validate an audio target.
- `units` and `target` are fixed choices in the descriptor and rendered as selects with friendly labels.
- Numeric threshold fields use placeholders and examples rather than defaults that imply a universal rule; `0` disables that bound.
- Output is JSON so CLI, chat and page all expose the same exact measured values.

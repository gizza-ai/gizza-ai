# video-dedup-frames competitor analysis (2026-08-22)

## Competitors scanned

| Tool / source | Table-stakes behavior | In model? | Decision for this tool |
| --- | --- | --- | --- |
| remove-duplicate-frames.vercel.app | Browser ffmpeg duplicate-frame remover with a simple mode that halves frame rate and an advanced mpdecimate mode. Exposes mpdecimate-style duplicate detection. | Yes | Ship a browser ffmpeg `mpdecimate` tool. Include a `max_fps` cap for the halve-first workflow and a higher-level sensitivity control so users do not need raw hi/lo thresholds. |
| AbeyIO frame extractor | Offline frame extraction tools often include duplicate removal when exporting frames, emphasizing local processing and output proof. | Partly | Keep this tool focused on producing a video, not extracted image frames. State that only consecutive duplicates are removed and leave frame-image export to extract-frames-style tools. |
| FFmpeg/mpdecimate tutorials | Common recipes use `mpdecimate`, then require VFR/CFR choices (`-fps_mode vfr` / `setpts`) because otherwise decimated frames can be reinserted. | Yes | Make `timing` explicit: keep original timing as VFR, constant frame-rate for editors, or compact to close gaps and shorten the clip. |
| Desktop/video editor workflows | Some editors remove or ignore repeated frames, but users expect MP4/WebM output, audio handling, and editor-friendly constant-rate output. | Yes | Offer `format=auto|mp4|webm`, copy/re-encode audio when safe, and drop audio for compact retiming to avoid sync lies. |

## Table-stakes parameters

- Duplicate sensitivity / threshold: in model. Implemented as `sensitivity` 1-100 mapped to mpdecimate `hi`/`lo`; default 50 matches ffmpeg defaults.
- Advanced changed-area threshold: in model. Implemented as `frac` 0.01-1 with default 0.33.
- Frame-rate halving / cap: in model. Implemented as optional `max_fps`, applied before `mpdecimate`.
- Timing choice: in model. Implemented as `keep`, `constant`, and `compact`.
- Output format: in model. Implemented as `auto`, `mp4`, `webm`.
- Whole-file non-consecutive visual duplicate clustering: out of model for this tool. It needs frame extraction/indexing and would not preserve timeline semantics.
- AI motion interpolation / repairing dropped frames: out of model. It needs an ML model and is the opposite of deduplication.
- GIF palette export: out of model here. Good GIF export requires a palette pass and already belongs to a dedicated video-to-GIF path.

## UX controls

- Use sliders for sensitivity and optional frame cap when schema bounds are available.
- Use enum selects for timing and format.
- Provide preset chips for default screen recordings, 60-fps capture capped to 30, compact slideshow, and editor-friendly constant-rate output.
- Page copy must explain the `mpdecimate did nothing` trap: dropping frames requires the right frame-rate/timing mode.

## Verification expectations

- Core tests assert exact ffmpeg argv for default MP4 and mode-specific options.
- Web/Playwright should assert the wasm `build_argv` surface and page deep-link controls.
- CLI generated example should parse and fail gracefully on an unreachable example URL.

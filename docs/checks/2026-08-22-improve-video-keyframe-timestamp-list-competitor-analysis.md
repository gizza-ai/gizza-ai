# video-keyframe-timestamp-list — competitor analysis (2026-08-22)

## Scope

The tool lists the keyframe / I-frame timestamps from the first video stream of a video so a user can plan precise seeks, stream-copy cuts, GOP checks, and segment boundaries. It measures only; it does not rewrite the video.

## Sources reviewed

- FFmpeg / ffprobe recipes surfaced by search results for extracting keyframe timestamps from video.
- Community command examples that use `ffprobe -skip_frame nokey -show_frames` with `pts_time` / `best_effort_timestamp_time`.
- Shell snippets that print timestamps, CSV rows, and keyframe intervals for HLS / seek troubleshooting.
- Existing in-repo video tools such as `video-scene-cut-diff`, `video-set-keyframe-interval`, and `video-fragmented-mp4` to avoid duplicating GOP-setting or scene-detection tools.

## Table-stakes capabilities

| Capability | In model? | Decision |
| --- | --- | --- |
| Read a normal video container and inspect the first video stream | yes | Uses the existing `Input::Video` + `AssetKind::Video` source-resolution pattern and ffmpeg-runtime. |
| Return exact keyframe timestamps | yes | ffmpeg `select='eq(pict_type\,I)',showinfo` emits one log line per I-frame; the core parses `pts_time:`. |
| Avoid requiring `ffprobe` | yes | Implemented with ffmpeg only, because the browser/chat runtime has ffmpeg-runtime, not ffprobe. |
| Multiple output renderings | yes | `format` enum supports `json`, `csv`, and `text`; the flat response also always includes `keyframes[]`. |
| Millisecond precision and higher/lower rounding | yes | `precision` integer supports 0–6 decimal places, default 3. |
| Keyframe interval / GOP spacing summary | yes | Response includes count, first, last, min/max/average gap, and per-row `gap_seconds`. |
| Frame numbers / byte positions | no | Common ffprobe output includes frame numbers and packet positions, but the ffmpeg `showinfo` log path is less stable for packet positions and the main user need is timestamp-based seeking/cutting. Listed as out-of-model for this version. |
| Visual timeline / downloadable spreadsheet page | no | This is a log-to-text measurement surface with no media output; the repo's generic ffmpeg page path is media-output oriented. Chat + CLI are the verified surfaces. |
| Batch multi-file comparison | no | Multi-input ffmpeg/page shapes are intentionally avoided in this repo; one video per invocation keeps the model simple. |

## UX / parameter decisions

- Default `json` output is structured enough for scripts, while `csv` supports spreadsheets and `text` supports pasteable seek lists.
- Default precision is milliseconds (`3`), matching practical frame-accurate seeking without noisy microsecond tails.
- The response includes both machine fields and the requested rendered `output` string so users do not have to rerun the tool to switch between analysis and copy-paste uses.
- The tool is not a duplicate of `video-set-keyframe-interval` (which creates a new video with a fixed GOP cadence) or `video-scene-cut-diff` (which detects shot changes between two edits). This tool reports existing I-frame positions in one source video.

## Verification implications

Use generated short MP4 fixtures with forced keyframes for exact-output CLI tests. Important advertised values are all three `format` enum choices and precision boundaries (`0` and `6`).

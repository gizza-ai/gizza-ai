# video-vfr-to-cfr competitor analysis (2026-08-16)

## Scope

Tool: convert variable-frame-rate video (phone, screen, game capture) to constant frame rate for editing, with optional audio re-locking.

## Competitor scan

| Source | Table-stakes found | In-model decision |
| --- | --- | --- |
| FFmpeg frame-rate documentation and community examples | CFR conversion is done during encode with output frame-rate control / sync mode; frames are duplicated or dropped to hit the target cadence; changing cadence requires re-encoding. | Built: `-fps_mode cfr`, optional `-r`, H.264 `yuv420p`, clear copy that explains duplicate/drop behaviour. |
| HandBrake-oriented VFR/CFR guides | Users expect common targets such as same-as-source/auto, 23.976, 24, 25, 29.97, 30, 50, 59.94 and 60 fps; quality presets are preferred to raw codec knobs. | Built: enum presets for `fps` and `quality`, with friendly page labels and example chips. |
| Desktop converter articles for VFR to CFR / audio desync | Main user problem is editor audio drift; successful tools highlight audio sync and editor compatibility, but many rely on desktop apps or proprietary processing. | Built: default-on `resync_audio` using `aresample=async=1:first_pts=0`; output stays local/browser-based. Proprietary repair/AI claims are out of model. |

## Parameters and defaults

| Capability | Default / options | Status |
| --- | --- | --- |
| Keep source nominal rate while making timestamps even | `fps=auto` | In model, built. |
| Explicit CFR presets | `23.976`, `24`, `25`, `29.97`, `30`, `50`, `59.94`, `60` | In model, built as `Param::enumv` and page select. |
| Quality control without exposing codec internals | `balanced` default, plus `high`, `small` | In model, built as CRF 20 / 18 / 24. |
| Audio drift correction | default on, optional off | In model, built as `aresample=async=1:first_pts=0`; off path stream-copies when container permits. |
| Container compatibility | keep mp4/mov/m4v/mkv, otherwise mp4 | In model, built using shared ffmpeg utility. |
| Batch conversion, timeline/project import, proprietary no-quality-loss mode | Not a pure single-file block capability. | Out of model; not built. |
| Automatic VFR detection report via ffprobe | Would require a separate probe/reporting surface before conversion. | Out of model for this conversion block; copy explains signs and ffprobe hint. |

## UX controls to match

- Use select controls for the frame-rate and quality presets so users do not type fragile rates.
- Keep audio re-locking as a default-checked checkbox because the primary user problem is sync drift.
- Include example chips for common workflows: same-as-source phone/screen capture, 60 fps gameplay, and PAL 25 fps with audio copy.
- Page copy must be generic and brand-free; no competitor names or copied marketing language.

# Competitor analysis — video-audio-loudness-compare (2026-08-01)

Tool function: measure the loudness of two recordings (video or audio), report
integrated LUFS / true peak for each, and state the loudness gap plus the gain
needed to level them.

## Landscape

### 1. FFmpeg `ebur128` / `loudnorm`
- FFmpeg's `ebur128` filter reports momentary, short-term, integrated loudness,
  loudness range, and true peak-style measurements for media files.
- The `loudnorm` filter is commonly used in two-pass normalization workflows and
  prints input integrated loudness, true peak, LRA, and threshold before applying
  gain to a target.
- These are powerful but command-oriented; comparing two files requires running
  the command twice and subtracting the results manually.

### 2. Desktop loudness meters / DAW plugins
- Tools such as broadcast loudness meters and mastering plugins focus on LUFS,
  true peak, LRA, momentary/short-term windows, and delivery targets.
- They are interactive and visual, but not ideal for quick CLI/chat workflows or
  batch comparisons between two exported videos.

### 3. Online loudness / normalization tools
- Web tools generally ask for an upload and report whether a file is above or
  below common platform targets; some normalize media server-side.
- For this repo, uploading full videos to a service is out-of-model; local pure
  Rust analysis of user-provided files/refs is the fit.

## Table-stakes → decisions

| Capability | Seen in | In/out of model | Decision |
| --- | --- | --- | --- |
| Integrated loudness in LUFS / LKFS | FFmpeg, loudness meters | in-model | Use ebur128 integrated loudness |
| True peak / sample peak | FFmpeg/loudnorm, meters | in-model | Report dBTP and dBFS per file |
| Loudness range + momentary/short-term max | FFmpeg ebur128, meters | in-model | Include LRA, max momentary, max short-term |
| Two-file comparison | manual workflow | in-model | Compute which file is louder and loudness_gap_lu directly |
| Gain-to-match suggestion | normalization workflows | in-model | `match_target=first|second|louder|quieter|target` with gain dB/linear |
| Headroom/clipping warning | mastering tools | in-model | Warn when suggested gain exceeds true_peak_ceiling |
| Common delivery target offsets | streaming/broadcast guidance | in-model | Include offsets for streaming -14, Apple Music -16, EBU -23, ATSC -24 |
| Video audio support | FFmpeg/ffprobe | in-model | Decode the first decodable audio track from MP4/MOV/MKV/WebM etc. using symphonia |
| Rewrite/normalize media | loudnorm tools | existing sibling tools | Not built here; this tool measures only. Use audio-normalize / loudness-matched-ab-prep for rewriting |
| Rich waveform/loudness timeline charts | GUI meters | out-of-model for this no-page two-file shape | No standalone page; chat+CLI JSON result only |

## Existing-block duplicate check

- `loudness-matched-ab-prep` measures two audio files and returns gain-matched WAV
  files in a zip; it does not accept video sources as `AssetKind::Any` and is a
  media-rewrite/prep tool.
- `loudness-spec-compliance` checks one audio file against one delivery spec.
- `video-audio-sync-offset-finder` compares timing, not loudness.

So this is not a semantic duplicate: it is the measurement-only, two-recording
video/audio loudness comparison surface.

> Original work only — competitor behaviour is paraphrased; no competitor copy,
> branding, or trademarks are reused.

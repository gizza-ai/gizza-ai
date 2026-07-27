# Competitor analysis — video-trim-black-frames (2026-07-25)

Tool function: detect and trim the leading and trailing fully-black frames from a video
(temporal edge trim), leaving the real picture. Distinct from `video-autocrop-bars`
(spatial: letterbox/pillarbox *bars* around the frame) and `video-silence-cut`
(audio-based). All competitor copy/branding is PARAPHRASED, never copied.

## Competitors scanned

1. **ffmpeg `blackdetect` filter (canonical CLI technique)** — the primitive every tool
   builds on. Options: `pixel_black_th`/`pix_th` (max pixel luma to count as black,
   default **0.10**), `picture_black_ratio_th`/`pic_th` (fraction of black pixels for a
   whole frame to count as black, default **0.98**), `black_min_duration`/`d` (min black
   run reported, default **2.0 s**). Reports `black_start`/`black_end`/`black_duration`
   timestamps to stderr per detected run.
2. **slhck/ffmpeg-black-split (PyPI `ffmpeg-black-split`)** — packaged tool that detects
   black periods and *splits* the video at them. Exposes exactly the three blackdetect
   knobs (`-d 2.0`, `-r 0.98`, `-t 0.1`), plus output-extension (default mkv),
   `--no-split`, `--no-copy` (re-encode instead of stream-copy), and a JSON timestamp
   dump. Splits *on* black rather than trimming edges.
3. **FarisHijazi deblack gist/repo** — detect via blackdetect, then rebuild the clip with
   `trim`+`concat` to drop black segments (including leading/trailing). Documents a real
   limitation: naive concat stalls after ~6-7 segments.
4. **GDELT blackdetect writeup** — practitioner tuning notes: uses `blackdetect=d=0.05:pix_th=0.10`
   (a *small* min-duration so short black runs are caught) and recommends post-filtering
   by `black_duration`. Confirms 2.0 s is too coarse for edge trimming; sub-0.1 s is the
   working default for catching brief intros/outros.

## Table-stakes params (each tagged, none dropped)

| param | competitors | our decision | tag |
| ----- | ----------- | ------------ | --- |
| pixel black threshold (`pix_th`) 0.10 | all | `pixel_threshold` number 0-1, default 0.10 | IN-MODEL ✓ descriptor |
| picture black ratio (`pic_th`) 0.98 | all | `black_ratio` number 0-1, default 0.98 | IN-MODEL ✓ descriptor |
| min black duration (`d`) | all (2.0), GDELT 0.05 | `min_duration` number 0-60 s, default **0.10** (edge-trim tuned, not 2.0) | IN-MODEL ✓ descriptor |
| which ends to trim | deblack (both), split (n/a) | `ends` enum both/start/end, default both | IN-MODEL ✓ descriptor |
| re-encode vs stream-copy | ffmpeg-black-split `--no-copy` | always re-encode (H.264 CRF 18 + AAC) for frame-accurate cut points; stated on page | IN-MODEL (chosen default) |
| output container | ffmpeg-black-split `-e mkv` | follow family `h264_out_ext` rule (keep mp4/mov/m4v/mkv, else mp4) | IN-MODEL ✓ |
| presets | GDELT tuning notes | `[[example]]` chips: default / brief black / dark-grey fades | IN-MODEL ✓ |

## Out-of-model / out-of-scope (listed, NOT built)

- **Removing mid-clip black segments / splitting on every black run** (ffmpeg-black-split,
  deblack) — a different tool by design; this one trims only the two EDGES per the backlog
  spec. A sibling `video-silence-cut`-style "cut all black" tool would own that. Noted, not built.
- **JSON timestamp export / batch directory processing** — needs a server/batch runner; out of the
  browser-local wasm model.
- **Closed-caption-assisted detection** (GDELT) — needs an external caption source; out of model.

## UX patterns matched

- Two-pass detect→trim (like `video-autocrop-bars`) with a friendly "no edge black frames
  detected" outcome instead of a pointless re-encode.
- Slider for the 0-1 thresholds; `<select>` for `ends`; preset chips.
- State the limits (25 MB cap, re-encode, edge-only scope) on the page.

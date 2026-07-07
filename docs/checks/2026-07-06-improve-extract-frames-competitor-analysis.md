# extract-frames — competitor analysis (2026-07-06)

Tool: **extract-frames** — sample frames from a video by fixed interval, fixed
fps, or scene-change points and tile them into a single **contact-sheet /
thumbnail-grid** image. ffmpeg tool; page + CLI are the working surfaces (chat
ffmpeg is unavailable).

## Why a contact sheet (and not a folder of PNGs)

The backlog description is "extract frames … as a batch of images." Gizza's page
renders exactly **one** output file, and ffmpeg cannot produce a ZIP, so the
literal "N separate PNGs → ZIP download" output is **out of model** on the page
surface. The standard, page-compatible realization of the same function is a
**contact sheet** (a.k.a. storyboard / thumbnail grid) — a single image tiling
the sampled frames. That is a widely shipped product category (browser-local
storyboard generators, `vcsi`, GDS Video Thumbnailer, MPC contact sheets), so it
keeps the tool honest to the "which frames, sampled how" intent while fitting the
one-file model. Single-frame-at-a-timestamp is already covered by the existing
`video-frame-extract` block, so this tool is deliberately the *batch/grid* case.

## Competitors surveyed (paraphrased — no copy/branding reproduced)

1. **Browser-local video thumbnail / storyboard generators** — "storyboard" tab:
   pick a grid (e.g. 4×4), a frame source (scene changes / keyframes / time
   intervals), generate a single sheet; all processing local, no upload.
2. **`vcsi` (CLI, video contact sheet)** — captures frames at time intervals,
   customizable grid, per-thumbnail size, a metadata header, and timestamp
   overlays; background/foreground colors.
3. **Frame-extractor web tools (frame-extractor.video / videotoframes /
   DuneTools)** — interval / fps / count / scene-detection selection; JPG or PNG
   per-frame output; **download all frames as a ZIP**, named by timestamp; local
   in-browser processing.
4. **"N evenly-spaced thumbnails" tools** — choose a count (e.g. 1–50); frames
   are taken at even spacing across the whole duration; PNG/JPG.
5. **Desktop contact-sheet software (GDS-style)** — grid size, background/border
   colors, and other cosmetic layout controls; batch over many files.

## Table-stakes → decision

| Table-stake | In/out of model | Where it landed |
|---|---|---|
| Selection: fixed **interval** (every N s) | in | `mode=interval`, `value`=seconds |
| Selection: fixed **fps** (N frames/s) | in | `mode=fps`, `value`=fps |
| Selection: **scene change** (threshold) | in | `mode=scene`, `value`=0–1 (always keeps frame 0) |
| **Grid** columns × rows | in | `columns` (1–8), `rows` (1–8) — sliders |
| **Thumbnail size** | in | `width` (16–800 px) — slider; height keeps aspect |
| **Background / gap color** | in | `background` (name or hex; color-picker control) |
| **PNG / JPG output** | in | `format` enum (png/jpg) |
| Local, no upload, no account | in (inherent) | browser wasm ffmpeg |
| Preset buttons (storyboard/fps/scene) | in | four `[[example]]` chips |
| **Per-frame download / ZIP of N images** | **out** | one-file page + ffmpeg can't zip — noted on page + FAQ (points at `video-frame-extract` for a single still) |
| **N frames evenly spaced across the whole duration** | **out (pure-argv)** | needs a duration probe not available to the pure `build_argv` path; approximated by interval mode. Considered, not built. |
| **Timestamp overlay per thumbnail** | **considered, rejected** | `drawtext` needs a bundled font file that @ffmpeg/core in the browser doesn't ship reliably; would fail on the primary (page) surface. Listed, not built. |
| **Metadata header banner** (filename/codec/res) | **considered, rejected** | needs `ffprobe` + `drawtext`/font, same wasm-font problem as above. |

Every surveyed table-stake is either in the descriptor or listed above — none
dropped silently.

## Feasibility spikes done before tagging

- Verified `select=eq(n\,0)+gt(scene\,T)` , `fps=1/N`, `fps=N`, and
  `tile=CxR:margin:padding:color` all run as a single raw-argv `-vf` token
  (no shell), including the backslash-escaped commas, on the local ffmpeg and
  (via the page Playwright run) on the browser @ffmpeg build.
- Confirmed `tile` emits a full padded grid at EOF even when fewer frames than
  `cols*rows` are sampled → dimensions are deterministic from grid + width +
  source aspect (asserted exactly in the page spec).
- Confirmed short-hex (`#f0a` → `0xFF00AA`) renders the requested margin color
  end-to-end (the "#f00 drew a white wave" class of bug is absent).

## UX patterns adopted

- Enum `<select>`s with friendly labels for `mode` and `format`.
- Range **sliders** for the bounded integers (columns, rows, width).
- Native **color** control (swatch + text) for the background.
- Four one-click **preset chips**: storyboard (every 2s), 1 fps, at scene
  changes, dense 6×4 JPG sheet.
- Stated limits on the page: the grid caps the frame count (first `cols*rows`
  sampled frames); scene mode needs visible changes; large videos are slower.

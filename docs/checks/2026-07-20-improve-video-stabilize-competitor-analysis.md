# video-stabilize — competitor analysis (2026-07-20)

Scan done BEFORE implementation (build-time design input, per create-next-tool).
All notes paraphrased; no competitor copy/branding reproduced.

## Competitors reviewed (top 3 reachable real tools)

1. **ezgif video stabilizer** — one-click tool explicitly built on ffmpeg's
   `deshake` filter. No user-facing parameters at all. Accepts a wide format
   list (mp4/webm/avi/mov/mkv/…), 200 MB cap, upload or URL input. Honestly
   states it suits mild-to-moderate shake and that severe footage needs
   desktop software.
2. **online-video-cutter.com stabilizer** — editor-style UI: timeline segment
   selection, preview-before-export, output format picker (30+ formats),
   Google Drive/Dropbox import, premium tier for 4 GB files and Full-HD
   export. No strength control surfaced.
3. **VidClean stabilizer** — three intensity presets (subtle / normal /
   strong), always outputs MP4, and hides shaky borders by slightly zooming
   in (crop-based border handling), with a note that stabilization zooms in.
   2 GB cap, no account, no watermark, 3-step progress UX.

(Also surfaced: TensorPix / Vmake / AVCLabs — AI/ML stabilizers, noted below
as out-of-model.)

## Table stakes → where they landed

| Table stake | Tag | Where it landed |
| --- | --- | --- |
| Stabilization intensity control (presets subtle/normal/strong) | in-model | `strength` 1–100 slider (default 25 = ffmpeg deshake's native radius) + three preset chips (Subtle 15 / Normal 30 / Strong+crop 60) |
| Hide shaky borders by slight zoom-crop (VidClean's default behavior) | in-model | `borders=crop` mode: `deshake edge=blank` + `scale`/`crop` chain, zoom 3–10 % growing with strength; spiked locally before committing (128→126 px exact) |
| Border fill alternatives (no-zoom option) | in-model | `borders` enum: mirror (default, no zoom), crop, blank, original — friendly `<select>` labels |
| One-click simplicity (ezgif) | in-model | defaults run with zero configuration (upload → stabilized mp4) |
| Multiple input containers (mp4/webm/mov/mkv…) | in-model | anything ffmpeg reads; mp4/mov/m4v/mkv keep container, others → MP4 (family `h264_out_ext` invariant); webm verified end-to-end |
| Honest "mild-to-moderate shake" limits statement (ezgif) | in-model | stated in page copy + FAQ |
| URL input (ezgif) | in-model | CLI/chat `url=` param (page is file-upload, family invariant) |
| AI/ML motion analysis (TensorPix, Vmake, AVCLabs) | out-of-model | needs an ML model; gizza is pure-Rust + ffmpeg |
| Two-pass analyze-then-transform stabilization (vid.stab) | out-of-model | needs two ffmpeg invocations + libvidstab, which the browser @ffmpeg/core build doesn't ship; `dispatch_ffmpeg` is single-invocation. deshake (single-pass, built into every ffmpeg) used instead |
| Timeline segment selection + preview editor | out-of-model | generated page is a single-shot form; video-trim / video-cut-segments already cover segment work |
| 2–4 GB files, cloud import (Drive/Dropbox), email notify, premium tiers | out-of-model | browser-local, no accounts; 25 MB in/out cap stated on page |
| Output format picker (30+ formats) | considered, rejected | video-transcode already exists; family invariant keeps the container or falls back to MP4 |

## Design decisions

- **Filter:** ffmpeg `deshake` (same engine ezgif advertises). During CLI
  verification discovered deshake rejects `rx`/`ry` not divisible by 16
  ("rx must be a multiple of 16"), so strength quartiles snap to the four
  accepted radii: 1–25→16 px, 26–50→32 px, 51–75→48 px, 76–100→64 px.
  Documented in the descriptor, page copy, and tests.
- **Crop & zoom math:** zoom z = 1 + (2 + 0.08·strength)/100 (1.02–1.10);
  `scale=trunc(iw·z/2)*2` then `crop=trunc(iw/z/2)*2` keeps dimensions even
  for H.264 and lands within ~2 px of the original size. Verified exactly:
  640×360 @ strength 60 → 638×358 (CLI), 128×128 → 126×126 (page).
- **Re-encode:** libx264 crf 20 preset medium; audio stream-copied when the
  container is kept, AAC when switching to MP4 (family invariant, shared
  `h264_out_ext`).

## Verification summary (all run, 2026-07-20)

- `cargo test --workspace`: 13 pass (core mapping/clamp/argv + drift-guard).
- CLI advertised-values matrix (Big Buck Bunny 360p public mp4/webm):
  mirror/crop/blank/original all run for real; strength 1 (cap), 100 (cap),
  101 (one-over → clamps); crop dims exact 638×358; webm→mp4/h264;
  invalid `borders=zoom` → "not supported (mirror|crop|blank|original)";
  generated CLI example verbatim → graceful HTTP 404 at fetch.
- Playwright: default mirror run decodes 128×128; `?borders=crop&strength=60`
  deep-link prefills + decodes exactly 126×126 (proves the chain ran in the
  browser ffmpeg build); build_argv matrix covers all four modes, radius
  snapping, webm→mp4, and the invalid-enum error.
- Hygiene gate: exit 0 (strict per-slug mode).

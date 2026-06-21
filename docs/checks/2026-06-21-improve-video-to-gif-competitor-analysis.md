# video-to-gif — competitor analysis (2026-06-21)

New tool: convert a section of a video into an optimized animated GIF. Three
surfaces verified: chat block (`wafer build` instantiated + drift-guard schema
test), CLI (real GIF produced from a public test video), and standalone page
(Playwright `tool-page-video-to-gif.spec.ts` passes — `data:image/gif` output).

## Implementation summary

- **High-quality palette**: single-pass `filter_complex` with
  `palettegen=stats_mode=diff` → `paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle`.
  This per-clip palette + dithering is the same quality technique the best
  competitors advertise ("two-pass palette optimization"), done in one ffmpeg
  pass so it fits gizza's single-dispatch model.
- **Section selection**: `start` + `duration` (input-seek `-ss`/`-t`).
- **Frame rate**: `fps` (default 12, range 0–60). Lower = smaller file.
- **Width**: `width` px, height auto via `scale=W:-2:flags=lanczos` (even, aspect
  preserved); 0 = keep source size.
- **Looping**: GIF loops forever (`-loop 0`).

## Competitors surveyed (top 5)

| Tool | Notable features | gizza parity |
| --- | --- | --- |
| FreeConvert | FPS, GIF size, trim | ✅ fps, width, start/duration |
| HighTool | FPS (15), width (480), quality slider, trimmer | ✅ fps/width/section; quality slider ≈ our fps+width controls |
| Ezgif | start/end times, frame rate, smoothness/size tradeoff | ✅ start/duration + fps |
| Kommodo | two-pass palette optimization, ffmpeg-wasm, in-browser | ✅ palette optimization, ffmpeg-wasm, in-browser, no upload |
| XConvert | width 480, fps 12, colors 128 to hit a size target | ⚠️ fps+width covered; explicit colour count below = gap (see below) |
| VidShift / ConvertICO | fps 10–24, width presets, platform presets (Discord/Slack/WhatsApp), max-file-size auto-tune | ⚠️ fps/width covered; presets + size-target auto-tune = gap |

## In-model gaps closed

The four meaningful, in-model controls — **section (start/duration), fps,
width, and palette optimization** — are all implemented. gizza additionally
matches the strongest privacy claim (fully local, nothing uploaded, works
offline once loaded), which several competitors (Kommodo, VidShift) lead with.

## Out-of-model / deferred gaps (NOT built)

- **Palette colour count (e.g. 128/64 colours)**: `palettegen=max_colors=N` is
  feasible as a future param; left out to keep the surface focused. The current
  per-clip palette already yields strong quality/size. (In-model — candidate for
  a follow-up improvement, not a competitor copy.)
- **Platform presets (Discord/Slack/WhatsApp) + max-file-size auto-tune**:
  these are UX conveniences that require an iterative re-encode loop
  (encode → measure → adjust → re-encode) which the single-dispatch ffmpeg model
  does not support headlessly. Deferred.
- **Visual in-page trimmer / scrubber**: the gizza page uses numeric start/
  duration fields (consistent with every other gizza video tool); a frame-scrub
  timeline UI is a site-wide page-driver feature, out of scope for one tool.

No competitor copy, branding, or trademarks were used. All copy is original.

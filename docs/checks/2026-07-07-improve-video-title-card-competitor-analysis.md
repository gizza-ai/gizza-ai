# video-title-card — competitor analysis (2026-07-07)

Tool function: overlay a styled **title / lower-third** caption onto a video over
a chosen time range. Scan done BEFORE implementing. All notes are paraphrased —
no competitor copy, branding, or trademarks reproduced.

## Competitors scanned (top real tools for "add title / lower third to video")

1. **VEED — Lower Third Maker** — browser editor; drag-to-position text, brand
   fonts + colors, background shapes/bars, timeline in/out points, animation
   presets.
2. **FlexClip — Lower Third generator** — template-driven lower thirds, custom
   text/color/font, animated variants, timeline placement.
3. **EchoWave — Add Text to Video** — text overlays + captions, font/size/color,
   background boxes, animation presets, custom timing (start/end), MP4 export.

(Also seen: Wave.video, Typito, Riverside "Add Text to Video", OpusClip "Add
Title" — same feature family: position, font, size, color, opacity, background,
start/end timing, animation.)

## Table-stakes (each tagged in-model = shipped in the descriptor, or out-of-model)

| Capability | Decision | Where it lives |
|---|---|---|
| Caption text | in-model | `text` (required; drawn literally via `textfile=`, no escaping) |
| Position on frame | in-model | `position` enum — 7 anchors (top/center/bottom × L/C/R) |
| Font size | in-model | `font_size` (8–400, slider) |
| Text color | in-model | `font_color` (CSS name or hex; color picker) |
| Background bar / box | in-model | `background` (bool, default on) + `background_color` + `background_opacity` |
| Timing (appear / disappear) | in-model | `start` / `end` seconds → ffmpeg `enable='between(t,START,END)'` |
| MP4 export / broad playback | in-model | H.264 + AAC, `+faststart`; webm/other → mp4 |
| Live in-browser, no upload | in-model | ffmpeg.wasm on the page; native ffmpeg on CLI |
| Font family selection | **out-of-model** | one bundled Liberation Sans Bold; multiple fonts would need a font-asset upload path |
| Animated in/out (fade/slide/typewriter) | **out-of-model** | drawtext is a static overlay; motion presets need a full editor/timeline |
| Prebuilt lower-third templates / graphics | **out-of-model** | shapes/logos/brand kits are an editor feature, not a single filter |
| Multiple simultaneous captions | **out-of-model** | one caption per run (re-run on the output to stack) |
| Drag-to-position (freeform x/y) | **out-of-model (approximated)** | replaced by 7 anchor presets — covers the common placements without a canvas UI |

### Feasibility spikes done before tagging (feasibility ≠ model fit)

- **drawtext + font available in the browser core?** — YES. Spiked
  `@ffmpeg/core@0.12.10` in headless Chromium: `drawtext` (libfreetype) is
  compiled in; `-filters` lists it. A real run with `fontfile=` + `textfile=` +
  `enable='between(t,…)'` + a box exited 0 and produced a larger output video.
  Both `0xRRGGBB@opacity` and `#RRGGBBAA` box-alpha forms work.
- **font supply** — the browser ffmpeg FS is empty, so the bundled font is
  shipped as an extra virtual-FS input; the native CLI service already writes
  every input to its temp dir. Made a shared `ArgvPlanWithInputs` +
  `dispatch_ffmpeg_inputs` so drawtext resolves identically on page + CLI.
- **text escaping** — sidestepped entirely by passing the caption as
  `textfile=` with `expansion=none`; `Don't Stop: 100% Live` renders literally.
  Verified in the spike.

## UX control patterns matched (from competitors → our declarative controls)

- Color pickers for text + background → `kind = "color"` (swatch + hex text).
- Font-size + opacity as sliders → `kind = "slider"` with `step`.
- Position as a labeled dropdown → `Param::enumv` + `[input.labels]`
  ("Bottom left (lower third)", …).
- Preset chips (a lower-third, a big centered title, a top banner, a
  first-3-seconds run) → four `[[example]]` chips.
- Start/end timing fields → plain number inputs (no upper bound is known before
  the clip is loaded, so no slider).

## Defaults chosen (match the common competitor defaults)

- position `bottom-center` (classic lower-third), font_size `48`,
  font_color `#ffffff`, background ON, background_color `#000000`,
  background_opacity `0.5`, start `0`, end `5`.

## Worked example (shipped on the page)

A 1280×720 clip + `Jane Doe — CEO`, bottom-left, 48 px, black bar @ 0.5, 0–5 s →
a same-size MP4 with the white caption on a half-transparent bar for the first
5 seconds, then the untouched clip. Set position Center + size 80 + no bar for a
big opening title.

Every table-stake above lands in the descriptor OR the out-of-model list — none
dropped silently.

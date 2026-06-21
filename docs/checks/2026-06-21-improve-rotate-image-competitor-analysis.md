# rotate-image — competitor analysis (2026-06-21)

## Tool summary
`gizza-ai/rotate-image` rotates an image clockwise by 90/180/270 degrees (lossless,
via `image` crate built-ins) or by any arbitrary angle (bilinear inverse-mapped
resample into an enlarged canvas, with a configurable background fill for the
exposed corners). Pure-Rust (`image` 0.25), so it runs on every backend including
the chat Service Worker. Returns a PNG.

Surfaces: **chat (LLM API) + CLI**. No page — image-bytes output has no page render
mode (same shape as `flip-image` / `normalize-image`).

Parameters:
- `angle` (number, default 90, range -360..360) — clockwise degrees; negatives go
  counter-clockwise. 90/180/270 are lossless.
- `background` (string, default `transparent`) — `transparent` / `white` / `black`
  or a hex `#rgb` / `#rrggbb` / `#rrggbbaa`. Only used for arbitrary angles.

## Competitors reviewed (top 5)
1. **Online Image Tools / Online PNG Tools (onlinetools.com, onlinepngtools.com)** —
   arbitrary angle in degrees or radians, custom background color, transparent fill
   for PNG output.
2. **Pinetools — Rotate image** — angle slider + numeric entry, background color
   picker, canvas expands to fit.
3. **Canva / Fotor / Pixlr — image rotator** — 90° increments + free-angle drag,
   auto-expand canvas, background fill (editor-level).
4. **ResizePixel / Imagy** — quick 90/180/270 buttons, format-preserving rotation.
5. **PixDuplicate / Elysia Tools** — any-angle rotation, choice of background color
   vs transparency, expand-canvas-to-fit.

## Capability diff (fit-to-model)

| Capability                                   | Competitors | gizza rotate-image |
|----------------------------------------------|:-----------:|:------------------:|
| 90 / 180 / 270 lossless rotation             | yes         | **yes**            |
| Arbitrary angle (degrees)                    | yes         | **yes**            |
| Clockwise positive / negative = CCW          | yes         | **yes**            |
| Expand canvas to fit rotated image           | yes         | **yes**            |
| Background fill: transparent                 | yes         | **yes**            |
| Background fill: named (white/black)         | yes         | **yes**            |
| Background fill: hex incl. alpha             | partial     | **yes** (#rrggbbaa)|
| Multi-format input (PNG/JPEG/WebP/GIF/BMP)   | yes         | **yes**            |
| Bilinear interpolation (smooth edges)        | yes         | **yes**            |
| Input via URL or prior-tool ref              | n/a         | **yes**            |

## Gaps closed
All in-model competitor capabilities are covered: lossless quarter-turns, arbitrary
angle with sign convention, auto-expanding canvas, and full background-fill options
(transparent, named, and hex with optional alpha — a small superset of most tools,
which only offer transparent or a solid picker). Bilinear sampling matches the
smooth-edge quality competitors advertise.

## Deferred (intentionally not built)
- **Crop/fit-to-original-dimensions mode.** Some editors let you keep the original
  canvas size and crop the rotated image instead of expanding. Expand-to-fit (the
  default of most rotate utilities and the least-lossy choice) is implemented; a
  `fit = expand|crop` toggle is a clean future addition but adds an output mode
  without a strong demand signal, so it is deferred rather than scope-creeping.
- **Radians input / angle slider UI.** Slider is a page-UI concern; this tool is
  chat+CLI only. Degrees cover the LLM/CLI use case.
- **Output-format selection (force JPEG/WebP).** Output is always PNG to preserve
  the transparent fill; format conversion is the separate `image-convert` tool.

## Verification (2026-06-21)
- `cargo test` — core: 8 unit tests pass (quarter-turns, 0/360 no-op, arbitrary-45
  canvas + corner fill, negative-angle normalize, color parsing, bad-image error);
  block: chat-schema drift guard passes.
- `wafer build` — block.wasm validates + instantiates (1403 KiB).
- CLI (`gizza tool rotate-image`): verified angle=90 (1104 B PNG), angle=45
  background=#ffffff (170×170 from 120×120, i.e. 120·√2), angle=-30, and the
  invalid-background error path.
- No page surface (image-bytes output, by design).

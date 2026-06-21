# colorblind-simulator — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/colorblind-simulator` — simulate how an image looks to people
with protanopia / deuteranopia / tritanopia. Pure-Rust (`image`). Image input →
image (PNG) output, so chat + CLI, no page (image-bytes output, like
sharpen-image / normalize-image).

## What competitors do

- **Online CVD simulators** (Coblis, various "colorblindness simulator" sites) —
  upload an image, see it under each deficiency. Useful, but **the image is
  uploaded** to a server.
- **Browser/devtools & design-plugin emulators** (Chrome rendering emulation,
  Figma/Stark plugins) — great while designing, but tied to that app and not
  scriptable for arbitrary image files.
- **Photoshop "Color Blindness" proof** — desktop, manual.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`image`) compiled to wasm: chat
   Service Worker and headless CLI. The image never leaves the device.
2. **All three dichromacies.** protanopia (red), deuteranopia (green — the most
   common), and tritanopia (blue), via the standard CVD simulation matrices
   applied per pixel; **alpha is preserved**.
3. **Accessibility check in one call.** Quickly verify whether a chart, UI, or logo
   stays legible for colour-blind users — from chat, the CLI, or as a step in a
   pipeline (`url`/`ref` in, PNG out, chainable).
4. **Lossless PNG output**, so the simulated image can be compared or annotated
   without recompression artifacts.

## Honest scope

- **Dichromacy simulation** (full protan/deutan/tritan) using the common
  approximate sRGB matrices — a faithful preview, not a clinically exact model
  (anomalous-trichromacy severity levels / Machado-2009 per-severity matrices are
  not exposed).
- **PNG output**; original format/metadata are not preserved.
- **No page** — image input + image-bytes output don't fit the page model
  (consistent with the other image-editing tools).

## Tests

4 core unit tests on **images assembled in-test**: output is a valid PNG of the
same dimensions with **alpha preserved**; deuteranopia maps pure red to the exact
matrix result; **white stays white** for all three types (matrix rows ≈ sum to 1);
and parse/erroring on a bad type and a non-image. Plus the block drift-guard schema
test. **CLI verified** end-to-end on a real image (Tux PNG → a valid simulated
PNG). `wafer build` instantiates the chat block (1.37 MiB).

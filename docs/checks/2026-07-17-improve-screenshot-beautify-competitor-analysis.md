# screenshot-beautify — competitor analysis (2026-07-17)

Tool function: wrap a screenshot in a polished frame — background padding, rounded
corners, soft drop shadow, and a gradient/solid backdrop — so raw screen grabs look
share-ready. Built as a pure-Rust `image` block (chat + CLI). Paraphrased scan below;
no competitor copy/branding/trademarks reproduced.

## Competitors scanned (WebSearch 2026-07-17)

Skimmed the top real tools returned for "screenshot beautifier padding rounded corners
shadow gradient background online":

1. **Screenhance** (screenhance.com/screenshot-beautifier) — gradient presets ("dozens"),
   custom gradient, solid color, custom-image backdrops; soft drop shadows; rounded
   corners; padding/spacing; device + browser-window frames (iPhone/iPad/MacBook/Pixel);
   aspect presets 16:9 / 1:1 / 4:3 / 9:16 / custom; export scale 1x/2x/3x; PNG/WebP/JPEG.
   Controls read as preset chips + visual sliders.
2. **Zumie** (zumie.io/tools/screenshot-beautifier) — padding slider (default **64px**);
   border-radius slider (default **12px**); drop-shadow toggle; **16 gradient presets**
   (Ocean, Sunset, Midnight, brand green…); top-bar selector (None / macOS Light / macOS
   Dark, with traffic-light dots); aspect chips (Auto / 16:9 / 4:3 / 1:1); exports 2x PNG.
   All processing is local/in-browser.
3. **VSPIC** (vspic.com/screenshot-beautifier) — padding, corner radius, shadow; gradient
   presets or custom background; device frame; export scale 50–200%; batch + ZIP. (Third
   pick after webutility.io returned HTTP 403 and was replaced, per the "replace an
   unreachable competitor" rule.)

Consensus defaults where stated: padding ≈ 64px, corner radius ≈ 12px, shadow on by
default, gradient backdrop by default, aspect Auto.

## Table-stakes params → decision (each ends in the descriptor OR the out-of-model list)

| Capability | In/out model | Decision |
|---|---|---|
| Padding around the shot | in-model | `padding` (px, default 64) |
| Rounded corners on the shot | in-model | `corner_radius` (px, default 16; alpha-mask rounded rect, anti-aliased) |
| Soft drop shadow | in-model | `shadow` (bool, default on) + `shadow_blur` (px) + `shadow_opacity` (0–1); gaussian-blurred silhouette offset down |
| Gradient backdrop | in-model | `background=gradient` (default) + `bg_color`/`bg_color2` + `gradient_angle` (linear lerp) |
| Solid-color backdrop | in-model | `background=solid` (uses `bg_color`) |
| Aspect-ratio presets | in-model | `aspect` = auto/16:9/4:3/1:1/9:16 (pads out to ratio, never crops) |
| macOS window title bar + traffic-light dots | in-model | `titlebar` = none/light/dark (drawn as a bar + 3 dots atop the card) |
| Gradient **preset** names (Ocean/Sunset/…) | in-model (as guidance) | expressed via the two color params + angle; described in `.describe()` and CLI examples rather than a fixed enum, so any brand hue is expressible |
| Custom-image backdrop | out-of-model | needs a **second** image source + full-cover compositing; deferred to keep a single image input. Listed, not built. |
| Device/photo frames (iPhone/MacBook bezels) | out-of-model | needs licensed device-bezel art assets; a plain macOS title bar is the in-model subset we ship. Listed, not built. |
| Export scale 1x/2x/3x + batch/ZIP | out-of-model (scope) | single-image transform; upscaling a raster shot adds no detail. Listed, not built. |
| Output format WebP/JPEG | partial | output is **PNG** — the shadow + rounded corners need an alpha channel, which JPEG can't carry. PNG is the correct single output. |

## UX control patterns competitors ship (for the page surface, N/A here)

Sliders (padding, radius, shadow), color pickers (backdrop), and preset chips (gradient
presets, aspect ratios, title-bar mode). These are **page-only** patterns. This tool
outputs image bytes, and in this repo image-bytes tools have **no page** (all page image
tools use the ffmpeg browser runtime; there is no pure-Rust image-output page renderer —
`tool.js` writes a pure-`wasm` result as text, and rounded-corner + drop-shadow +
gradient compositing is unreliable as an ffmpeg filtergraph). So screenshot-beautify
ships as **chat + CLI**, exactly like `image-border-frame` / `image-round-avatar` — a
documented pattern (create-next-tool references/page-patterns.md: "image-bytes output →
build_media_envelope, chat+CLI, NO page"). The competitor sliders/chips map onto the
descriptor's numeric/enum params, which the chat + CLI surfaces expose directly; no page
means no Playwright page spec — verification is `cargo test --workspace` (incl. the
schema drift-guard the chat surface consumes) + the CLI.

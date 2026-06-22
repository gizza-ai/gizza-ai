# image-contain-fit — competitor analysis (2026-06-22)

## Tool
`image-contain-fit` — fit (letterbox) an image inside a target width × height
preserving its aspect ratio, padding the leftover space with a chosen background
colour so the output is exactly the requested size. The scaled image is centred.
Pure Rust (`image` crate) — runs on the chat skill and the CLI; nothing is
uploaded to a third party. Image-bytes output, so no standalone page (same as
flip-image / normalize-image — the page driver has no image-bytes render mode).

## Distinct from existing gizza blocks
- `image-resize` (ffmpeg) has a `contain` *fit* but it only scales down inside a
  box preserving aspect — it does **not** pad to the exact target size, so the
  output dimensions vary. `image-contain-fit` always returns the exact WxH canvas
  with the bars filled in.
- `image-crop` removes pixels; `image-border-frame` adds a uniform border around
  the *existing* size. Neither produces an aspect-fit letterbox to a target size.

## Top competitors surveyed
1. **Online Image Tools — Pad an Image** (onlinetools.com/image/pad-image) —
   drag-drop, choose side(s), padding width and colour.
2. **Vayce — Image Padding Adjuster** — exact width/height, anchor point, and a
   background colour (or transparent) for the added pixels.
3. **Online Mini Tools — Pad Image** — customisable padding, background colours,
   9 positions (center/corners/edges), real-time preview.
4. **editingtools.io — Letterbox Generator** — adds top/bottom or left/right bars
   to fit an image into target dimensions.
5. **onlinepngtools — Change PNG Canvas Size** ("Fit to Canvas") — resizes so the
   image touches two opposite canvas edges, proportions unchanged, may scale up or
   down, empty area filled with a chosen colour.

Sources:
- https://onlinetools.com/image/pad-image
- https://vayce.app/tools/image-padding-adjuster/
- https://onlineminitools.com/pad-image
- https://editingtools.io/letterbox/
- https://onlinepngtools.com/change-png-canvas-size

## Feature diff

| Capability | Competitors | image-contain-fit | Notes |
|---|---|---|---|
| Exact target W×H output | yes | yes | output is always exactly width×height |
| Aspect-ratio-preserving fit (contain) | yes | yes | Lanczos3 downscale/upscale |
| Pad colour (hex) | yes | yes | `#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa` |
| Named colours | some | yes | white/black/transparent/red/green/blue/gray |
| Transparent padding | yes (Vayce) | yes | alpha preserved in PNG |
| Centred placement | yes | yes | bars split evenly on both sides |
| Upscale toggle (don't enlarge small images) | yes (Fit to Canvas) | **yes** (`allow_upscale`) | off → small image stays 1:1, centred |
| In-browser / offline / no upload | some | yes | pure Rust, nothing uploaded |
| Chat + API + CLI surfaces | no | yes | unique to gizza |

## Gaps closed this pass
Built fresh this pass with the in-model competitive feature set already covered:
- **Exact-size letterbox** to a target W×H (the core differentiator vs the existing
  `image-resize` contain mode).
- **Arbitrary pad colour** — hex (3/4/6/8 digits) and common names, including
  `transparent` (alpha preserved), matching Vayce / onlinepngtools.
- **`allow_upscale` toggle** — matches onlinepngtools "Fit to Canvas" behaviour of
  optionally not enlarging an already-small image (it stays centred at 1:1 inside
  the padding) — a capability the simpler padders lack.

## In-model gaps deliberately NOT built
- **Anchor / 9-position placement** (Online Mini Tools): competitors let you anchor
  the image to a corner/edge instead of centring. Centred letterbox is the standard
  "contain" behaviour and the overwhelming default; a position param could be a
  later enhancement but is not core to letterboxing.
- **Per-side manual padding widths** (onlinetools Pad an Image): that is a different
  operation (add N px to chosen sides of the *current* image), already closer to
  `image-border-frame`; this tool's contract is fit-to-target.
- **Live preview UI**: a browser-side preview is a page-surface affordance; this is
  an image-bytes tool with no page, by design.

## Out-of-model (not built — would need other capabilities)
- None applicable: the whole feature is pure-Rust image compositing, fully in-model.

## Verification (all green)
- `cargo test --workspace` — 10 tests pass (9 core: colour parsing, contain-size
  math, exact-size output, pad colour, transparent alpha, error cases; + 1 chat
  schema drift-guard).
- `wafer build` — chat block instantiates and validates (1450.8 KiB).
- `cargo run --manifest-path tools/generator/Cargo.toml -- .` — generator renders
  all 155 tools without abort.
- CLI: `gizza tool image-contain-fit url=<qr png> width=200 height=400
  background=black` → 200×400 PNG (verified byte dimensions); `background=transparent
  allow_upscale=false` works; an invalid colour name returns a clear error.
- No page surface (image-bytes output → no page render mode), stated explicitly.

No competitor copy, branding, or trademarks were used.

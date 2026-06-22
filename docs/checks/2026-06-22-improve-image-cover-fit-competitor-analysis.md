# image-cover-fit — competitor analysis (2026-06-22)

## What the tool does

Scales and centre-crops ("cover" resize, CSS `object-fit: cover` / `background-size: cover`)
an image so it completely fills a target `width` x `height` while preserving aspect ratio,
then crops the overflow around a chosen `anchor` (gravity). Output is exactly the requested
size with **no padding** — the complement of the existing `image-contain-fit` (letterbox).
Pure-Rust (`image` crate), runs on all backends incl. the chat service worker. Surfaces:
chat + CLI (image-bytes/PNG output → no page, like `image-contain-fit`, `flip-image`).

Params: `url`/`ref` (source), `width`, `height`, `anchor`
(center/top/bottom/left/right/top-left/top-right/bottom-left/bottom-right, default center),
`allow_upscale` (default true).

## Top competitors surveyed

1. **ImageResizer.com – Crop Image** — exact-pixel crop with selectable aspect-ratio presets,
   no distortion.
2. **img2go.com – Crop Image** — live pixel-dimension readout; presets for Instagram Square
   (1:1), Portrait (4:5), Landscape (1.91:1), Facebook cover, Twitter header, Pinterest pin.
3. **RedKetchup Image Resizer** — combined resize + crop, pixel control.
4. **PicResize.com** — social-target presets (YouTube thumbnail, IG profile/post, FB/LinkedIn).
5. **Adobe Express / Fotor** — preset social aspect ratios, manual width/height entry, quality-preserving.

## Gap analysis (fit-to-model)

| Competitor capability | In gizza model? | Status |
|---|---|---|
| Exact target pixel dimensions | Yes — `width`/`height` required | ✅ covered |
| Preserve aspect ratio, fill the box (cover) | Yes — core "cover" rule | ✅ covered (the tool's whole point) |
| Crop gravity / which part to keep | Yes — `anchor` (9 gravities + N/S/E/W + centre/middle aliases) | ✅ covered (key differentiator vs a centre-only cropper) |
| Don't upscale small images | Yes — `allow_upscale=false` clamps at 1:1 | ✅ covered |
| Quality scaling | Yes — Lanczos3 resample | ✅ covered |
| No watermark / no quality loss / no sign-up | Yes — local PNG, lossless container | ✅ inherent |
| Social-media **named presets** (IG 1:1, FB cover, etc.) | Partial | ⚠️ deliberately omitted — a preset is just a `width`x`height`; the chat LLM and CLI user already supply explicit dimensions, so a baked-in preset list would duplicate input the caller controls and go stale as platform sizes change. The tool description steers callers to pass the exact size. |
| Interactive drag-to-position crop box | UI-only | ⛔ out of model — gizza tools are headless (chat/CLI), no interactive canvas. The `anchor` param is the non-interactive equivalent. |
| Circle / rounded crop | Separate tool | ⛔ out of scope — `image-round-avatar` already covers circular crops. |
| Free-form rectangular crop at x,y | Separate tool | ⛔ out of scope — `image-crop` already covers explicit-rectangle cropping. |

## Decisions

- **No new gaps to close in-model.** The core cover semantics, exact sizing, 9-way crop gravity,
  no-upscale guard, and high-quality resampling already match or exceed the surveyed cover/crop
  tools' non-UI capabilities.
- **Named social presets** were considered and rejected: they restate caller-supplied dimensions
  and rot over time; explicit `width`/`height` is strictly more flexible and the description
  already nudges callers to it.
- **Interactive crop UI / circle / freeform-rectangle** are out of the headless model or already
  served by sibling tools (`image-round-avatar`, `image-crop`).
- NO competitor copy, branding, or trademarks were used.

## Verification (this run)

- `cargo test --workspace`: 12 tests pass (11 core incl. anchor/cover-size/no-padding/anchor-crop-region + 1 drift-guard).
- `wafer build`: OK, `gizza-ai/image-cover-fit v0.1.0` block.wasm validates.
- CLI: `gizza tool image-cover-fit url=… width=200 height=200 anchor=center` → exact 200x200 PNG;
  also verified `anchor=top allow_upscale=false` (100x400) path.
- No page surface (image-bytes output) — same as `image-contain-fit`.

Sources: [ImageResizer](https://imageresizer.com/crop-image), [img2go](https://www.img2go.com/crop-image), [RedKetchup](https://redketchup.io/image-resizer), [PicResize](https://picresize.com/en/crop-images), [Adobe Express](https://www.adobe.com/express/feature/image/crop), [Fotor](https://www.fotor.com/features/crop)

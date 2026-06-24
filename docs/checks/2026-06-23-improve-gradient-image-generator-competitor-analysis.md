# gradient-image-generator — competitor analysis & improvement check (2026-06-23)

## Tool

`gradient-image-generator` — create a solid, linear-, or radial-gradient **image**
(PNG raster or SVG vector) at a chosen size, for backgrounds, hero banners,
wallpapers, and placeholders. Pure-Rust (`image` PNG encoder + hand-built SVG),
so it runs on all backends incl. the chat Service Worker. Surfaces: **chat + CLI**
(image-bytes output → no standalone page, same as `qr-code-generator` and the
chart tools).

## Distinction from existing blocks (not a dup)

- `css-gradient-generator` — emits a CSS `background-image: linear-gradient(...)`
  **text declaration**, not a raster/vector image. Different surface (code vs a
  downloadable image file). Kept distinct.
- `text-image-card` — renders **text** onto a card with a themed background
  gradient; the gradient is a fixed theme, not user-driven, and the output is a
  text card, not a plain gradient. Distinct.
- `color-palette-generator` / `color-shades-generator` — produce colour **lists**,
  not images. Distinct.

## Surface verification (Phase 1)

- **chat**: `wafer build` validates + instantiates the wasm32-wasip1 block (OK,
  461.5 KiB). The block returns an `image/png` or `image/svg+xml` data-URL envelope
  via `build_media_envelope`.
- **CLI**: `gizza tool gradient-image-generator …` verified for:
  - linear PNG (200x100, angle 0) → valid PNG magic bytes `89 50 4E 47`.
  - radial SVG (120x120) → valid `<radialGradient>` SVG.
  - solid PNG (first stop only).
  - error path (`colors=#zzz`) → clear error message, exit 1.
- **page**: none — image-bytes output has no page render mode (documented pattern,
  consistent with `qr-code-generator`).

## Competitor scan (top tools for "gradient image generator / background generator")

Surveyed common online gradient-image / background generators (e.g. CSSGradient.io
image export, Gradienta, Coolors gradient maker, Photopea/Canva gradient fill,
WebGradients, MagicPattern gradient generator) for the **capability set** an
image-output gradient tool is expected to cover. (No copy, branding, names, or
trademarks reproduced — only feature categories compared.)

| Capability | Competitors | This tool | Gap action |
|---|---|---|---|
| Linear gradient | yes | yes | — covered |
| Radial gradient | yes | yes | — covered |
| Solid / flat fill | most | yes | — covered |
| Arbitrary direction/angle (linear) | yes | yes (0–360°, clockwise) | — covered |
| Multi-stop (2+ colours, evenly spaced) | yes | yes (unlimited stops) | — covered |
| Custom pixel dimensions | yes | yes (1–4096 per side) | — covered |
| Raster export (PNG) | yes | yes | — covered |
| Vector export (SVG) | some | yes | — covered |
| Alpha / transparency in stops | some | yes (#rgba / #rrggbbaa) | — covered |
| Conic gradient | some | no | out of model for this image tool — kept to solid/linear/radial; conic mesh rasterisation is a larger surface and `css-gradient-generator` already covers conic in CSS. Documented, not built. |
| Mesh / noise / grain texture | a few premium tools | no | out of scope — would need a noise/mesh engine; not a core gradient capability. Documented, not built. |
| Per-stop custom positions (non-even) | some | no (even spacing) | minor: even spacing covers the dominant use; explicit positions are a `css-gradient-generator` feature. Documented. |
| Curated presets gallery | several | no | a UI-gallery feature; the chat/CLI model exposes full colour control instead. Out of model. |

## Gaps closed this run

The initial implementation already covers every **in-model** capability that the
competitors expose for an image-output gradient tool:

- multi-stop gradients (not just 2 colours),
- alpha-channel stops (`#rgba` / `#rrggbbaa`) so the PNG/SVG can be semi-transparent,
- both PNG and SVG export,
- full 0–360° linear direction,
- solid fill as a first-class mode,
- generous size range (1–4096) with clamping rather than erroring on out-of-range.

No additional in-model gaps were found that warranted a code change.

## Out-of-model / intentionally not built

- **Conic gradients** — `css-gradient-generator` covers conic in CSS; conic *image*
  rasterisation is a separate, larger surface. Deferred.
- **Mesh gradients, noise/grain overlays, preset galleries** — UI/engine features
  beyond a deterministic gradient compute tool.
- **Per-stop custom positions** — even spacing covers the common case; non-even
  positions belong to the CSS code tool.

## Result

Built + verified on chat (wafer instantiate) + CLI (PNG/SVG/solid/radial/error).
Drift-guard schema test passes. No page surface (image-bytes pattern). Not a
duplicate of `css-gradient-generator` (code) or `text-image-card` (text card).

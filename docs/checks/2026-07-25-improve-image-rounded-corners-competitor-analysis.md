# image-rounded-corners — competitor analysis (2026-07-25)

Tool function: round the corners of an image at a chosen radius and export a
transparent PNG (the image keeps its original dimensions/aspect ratio — corners
are masked, the image is NOT cropped to a square). Distinct from the existing
`blocks/image-round-avatar`, which center-crops to a **square** and produces a
circle/rounded-square avatar. This tool preserves the full rectangular image
(banners, screenshots, cards).

## Scan (paraphrased — no copied copy/branding/trademarks)

Top competitors reviewed (function search, top results):

1. **onlinepngtools.com/round-png-corners** — rounds PNG corners; lets you
   control the radius of every vertex (per-corner control); transparent output.
2. **squareanimage.com/roundedcorners** — radius slider 0–50, where 50 fully
   rounds the short side into a pill/circle; transparent PNG; runs in-browser.
3. **imageonline.io/rounded-corners** — radius expressed as 0–50% of the
   smallest dimension; transparent PNG output.
4. **image-resizer.net/tools/rounded-corners** — round all four corners together
   OR individually; make circles, cards, custom shapes.
5. **layercy.com/tools/round-corners-image** — subtle curve → full circle;
   downloadable transparent PNG; browser-local (no upload).

## Table-stakes → decisions

| Capability | Competitors | Our decision |
|---|---|---|
| Adjustable corner radius | all | **built** — `radius` param |
| Radius as % of smallest side (0–50, 50 = pill) | squareanimage, imageonline | **built** — `unit = px \| percent` |
| Transparent PNG output | all | **built** — default; PNG always |
| Selective corners (round only top/bottom/left/right) | onlinepngtools, image-resizer, notchtools | **built** — `corners = all \| top \| bottom \| left \| right` |
| Full circle/pill at max radius | squareanimage, layercy | **built** — `unit=percent` radius 50 rounds the short side fully |
| Anti-aliased edges | implicit (all) | **built** — 1px coverage AA on the arc |
| Background color fill instead of transparency | some (card-on-color use) | **built** — `background` (transparent default, or hex / named color) |

## Considered, NOT built (out of model or rejected)

- **Independent radius per individual vertex** (onlinepngtools controls each
  vertex separately) — rejected for schema simplicity; the `corners` selector
  covers the common presets (card with only the top rounded, pill via left/right)
  at one shared radius. A single-corner (`tl`/`tr`/`bl`/`br`) mode could be added
  later if demanded.
- **Interactive live-drag slider preview** — a UI affordance of the hosted sites;
  this repo ships chat + CLI surfaces for image-bytes tools (no standalone page —
  the page file-input path is ffmpeg-only), so there is no live preview here.
- **Adding a drop shadow / border together with rounding** — separate concerns
  already covered by other blocks (`image-border-frame`, `image-document-shadow-remove`).
- **Batch / ZIP of many images, accounts, cloud storage** — out of gizza's
  browser-local, no-account, single-input model.

## Surfaces

Image input + image-bytes (PNG) output → **chat + CLI only, NO standalone page**
(image-bytes have no page render mode; the page file-input path is ffmpeg-only —
same shape as `image-round-avatar`, `add-text-to-image`, `image-pixelate-censor`).

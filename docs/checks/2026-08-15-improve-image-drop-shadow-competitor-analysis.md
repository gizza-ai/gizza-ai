# image-drop-shadow — competitor analysis (2026-08-15)

Scan run **before** implementing, per `/create-next-tool` step 3 / `/improve-tool` phase 2.
All findings are **paraphrased**; no competitor copy, branding, or trademarks are reproduced,
and nothing here is copied into the page.

## Tools scanned (top real results for "add drop shadow to transparent PNG")

| # | Tool (paraphrased role) | Controls it exposes |
|---|---|---|
| 1 | Online PNG Tools — "add shadow to PNG" | offset X, offset Y, blur radius, shadow color (incl. semi-transparent), **padding** (empty pixels around image + shadow). PNG out. Daily free-plan cap on the hosted service. |
| 2 | ImageOnline — shadow generator | blur `0–50px` (default 10), offset X/Y `-50..50` (default 5/5), shadow color picker, opacity `0–100%` (default 50). Output PNG / JPG / WebP. States a `4096×4096` resolution ceiling. Explicitly **no** spread, **no** background color, **no** canvas padding. |
| 3 | Freetoolio — add drop shadow to image | polar controls: angle, distance, blur, opacity, plus shadow scale and shadow "squash" (Y). Shadow color. Requires a transparent PNG/WebP cutout. Advises opacity ~30–50% for a modern look. |
| 4 | Cleanor — add shadow to cutout | shadow type (drop / soft), offset X `-64..64` (default 14), offset Y `-64..64` (default 18), blur `0..80` (default 26), opacity in 0.05 steps (default 0.32), shadow color hex (default `#111111`). PNG out with partial alpha. **Documents that the canvas is never expanded, so shadows falling outside the frame are clipped.** |
| 5 | BestPNG — shadow tool | blur radius, X/Y offset, shadow color, opacity; transparent **or** colored canvas background. Recommends blur 20–40 px, offset 10–20 px, dark gray `#333` / black at 30–50% opacity. Single file at a time. |

Reference for parameter semantics: the CSS `box-shadow` / `filter: drop-shadow()` model
(offset-x, offset-y, blur radius, spread, color) — `drop-shadow()` follows the alpha channel
(the cutout silhouette), which is exactly the behaviour these tools implement, while
`box-shadow` shadows the rectangular box. Ours follows the alpha channel.

## Table-stakes → decision

| Capability | Seen in | Decision | Where it landed |
|---|---|---|---|
| Horizontal offset | 1,2,3(as angle+distance),4,5 | **in-model** | `offset_x`, px, default 12, `-500..500` |
| Vertical offset | 1,2,3,4,5 | **in-model** | `offset_y`, px, default 16, `-500..500` |
| Blur radius | 1,2,3,4,5 | **in-model** | `blur`, CSS-style radius px, default 24, `0..400` (mapped to a Gaussian sigma of blur/2, matching the CSS `drop-shadow()` convention) |
| Shadow color | 1,2,3,4,5 | **in-model** | `color`, hex or name, default `#000000`, page `kind = "color"` |
| Shadow opacity | 2,3,4,5 | **in-model** | `opacity`, percent, default 35 (mid-point of the 30–50 % that two competitors recommend), `0..100` |
| Canvas padding / no clipping | 1 (has it), 4 (documents the lack) | **in-model — differentiator** | `padding` px, default `0` = **auto**: enough room for the blur reach + offset so the shadow is never clipped |
| Keep the original frame (clip the shadow) | 4 (its only behaviour) | **in-model** | `clip_to_original` boolean, default `false` — checked keeps the exact input dimensions |
| Background color behind the result | 5 | **in-model** | `background`, default `transparent`, page `kind = "color"` |
| Output PNG / JPG / WebP | 2, (5 implied) | **in-model** | `format` enum `png|webp|jpg`, default `png`; `jpg` cannot hold alpha so a transparent background is flattened to white (documented on the page and in the `.describe()`) |
| Preset styles / recommended settings | 3, 5 (as prose advice) | **in-model** | `[[example]]` preset chips: Soft product shadow, Subtle UI card, Long cast shadow, Hard sticker edge, Colored glow, Keep original size |
| Stated resolution ceiling | 2 (`4096×4096`) | **in-model** | documented page limit; ours is bounded by the 8 MB input cap rather than a pixel count |

## Out of model (listed, deliberately not built)

- **Spread / grow-shrink radius** (CSS `box-shadow`'s 4th length). None of the five image tools
  ship it (ImageOnline lists it as explicitly absent); it needs a morphological dilate/erode of
  the alpha channel before the blur, which the browser ffmpeg build does not expose reliably.
  Approximating it by boosting the blurred alpha would silently change opacity instead, so it is
  omitted rather than faked.
- **Angle + distance polar controls** (tool 3) — the same two degrees of freedom as
  `offset_x`/`offset_y`, which is the form every other tool and CSS itself uses. Adding a second,
  redundant coordinate system would double the params for no new capability.
- **Shadow scale / squash — "ground" perspective shadow** (tool 3). This is a projected/skewed
  shadow, not a cast one; it needs a perspective transform of the alpha silhouette plus a
  gradient falloff. Out of scope for a single filtergraph, and a different tool shape.
- **Multiple stacked shadows / ambient occlusion** (tool 4 documents the lack too) — would need a
  repeat-count param and N chained branches; a second pass through the tool composes them today.
- **Background removal** (several tools bundle it) — already a separate gizza tool
  (`image-background-remove-ai`), so chaining is the right answer, not duplicating it.
- **Clipboard copy of the result** — already provided generically by the tool-page runtime's
  "Copy image" button for every image-output tool, so nothing tool-specific is needed.

## Notes on fit

The effect is implemented as a single ffmpeg filtergraph (no probing, so the argv is built purely
from the params, shared by the CLI and the page):

```
format=rgba,pad=<expand>,split=2[fg][s0];
[s0]colorchannelmixer=aa=<opacity>,format=gbrap,gblur=sigma=<blur/2>,
    lutrgb=r=<R>:g=<G>:b=<B>,crop=<shift>,pad=<shift>[sh];
[sh][fg]overlay=0:0:format=auto
```

Every filter in the chain (`format`, `pad`, `split`, `colorchannelmixer`, `gblur`, `lutrgb`,
`crop`, `overlay`) is already exercised by shipped gizza pages (`image-vignette`,
`image-bg-replace`, `video-blur-region`), so the browser `@ffmpeg/core` build carries them.
Verified natively before implementing: a 200×120 transparent-PNG cutout with blur 16 / offset
12,18 / opacity 50 % / red shadow produced transparent corners, an untouched opaque subject, and
a soft red shadow at alpha ≈ 105 — i.e. the alpha-channel silhouette is what casts the shadow.

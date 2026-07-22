# before-after-slider — competitor analysis (2026-07-22)

Scope: a browser-local, no-account generator that takes two image sources (URL or
`data:` URI) plus options and emits **one self-contained HTML blob** (inline CSS + JS,
no external libraries) implementing an interactive before/after comparison slider.
All competitor notes are **paraphrased** — features/defaults only, no copied copy or
branding.

## Competitors surveyed

1. **img-comparison-slider** (sneas) — standards-based Web Component (Custom Elements +
   Shadow DOM). Horizontal/vertical, drag-anywhere or handle-only, hover mode, keyboard
   arrows (default on), slotted per-side labels + custom handle, responsive, touch, many
   per page. Defaults: `value` 50, `hover` false, `direction` horizontal.
2. **JuxtaposeJS** (Knight Lab) — hosted no-code generator; two image URLs → iframe embed
   / hosted link (not a self-contained file). Orientation toggle, adjustable start,
   per-image labels + credit line, optional load animation, responsive.
3. **TwentyTwenty** (ZURB) — jQuery plugin, CSS-clip slider. Horizontal/vertical,
   before/after labels, overlay hint, move-on-hover, handle-only, click-to-move, touch.
   Defaults: `default_offset_pct` 0.5, `orientation` horizontal.
4. **image-compare-viewer** (Kyle Wetton) — zero-dep vanilla JS for photo grading.
   Horizontal/vertical, drag + hover-start, labels (optionally hover-only), circular
   handle + blur, smoothing, fluid mode, touch, multi-instance. Defaults: `controlColor`
   #FFFFFF, `startingPoint` 50, `showLabels` false, `verticalMode` false.
5. **cocoen** (koenoe) — minimal touch-first vanilla JS (rAF). Horizontal/vertical, drag +
   touch, configurable divider color + start, auto-parses every `.cocoen` on a page.
   Defaults: `start` 50, light divider color.

## Comparison table

| Feature | img-comp-slider | Juxtapose | TwentyTwenty | image-compare-viewer | cocoen | **ours** |
|---|---|---|---|---|---|---|
| Horizontal / vertical | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | **✓ / ✓** |
| Start position (default) | 50 | adj. | 0.5 | 50 | 50 | **50** |
| Per-side labels | slots | ✓ | ✓ | opt | – | **✓ (custom text, toggle via empty)** |
| Hover-to-move | ✓ | – | ✓ | ✓ | – | **✓** |
| Keyboard arrows | ✓ | – | – | – | – | **✓ (+ Home/End, Shift=×5)** |
| Handle/divider color | CSS | – | Sass | ✓ | ✓ | **✓** |
| Touch support | ✓ | ✓ | ✓ | ✓ | ✓ | **✓ (pointer events)** |
| Multiple per page | ✓ | ✓ | ✓ | ✓ | ✓ | **✓ (auto-init all)** |
| Responsive/fluid | ✓ | ✓ | ✓ | ✓ | ✓ | **✓ (+ optional max width)** |
| Deps | none | none | jQuery | none | none | **none** |
| Output | npm/CDN markup | iframe/hosted | npm/CDN | npm/CDN | npm/CDN | **self-contained HTML file or embed snippet** |

## Table-stakes (met)

Two overlaid images + draggable divider · configurable start position (default 50) ·
horizontal & vertical · toggleable per-side labels · responsive sizing preserving aspect ·
touch + mouse · customizable handle/divider color · multiple sliders per page · accepts two
image sources (URL/data URI) · handle drag-guard (`user-select:none`, `pointer-events:none`
images). **All covered.**

Differentiators we ship: **keyboard accessibility** (arrows + Home/End + Shift for big
steps, ARIA slider role) that only one competitor matches; a **document vs embed** output
switch (a full openable page *or* a paste-anywhere snippet); and a security guard that
rejects `javascript:`/non-image `data:` sources and HTML-escapes labels + `src` so the
generated file can't smuggle script.

## In-model vs out-of-model

**In-model (built):** two-image drag-to-wipe overlay (CSS `clip-path` + ~30 lines JS),
start-position %, horizontal/vertical, custom before/after labels, hover-to-move, keyboard
control + ARIA, divider/handle color, fluid + optional max-width, touch, multiple per page,
URL or data-URI sources embedded directly, document or embed output.

**Out-of-model (considered, not built):** uploading/**hosting** user files to get CDN URLs
(we inline a `data:` URI or reference a URL instead); shareable **hosted permalinks** /
iframe / oEmbed publishing (needs a server); **accounts, saved projects, analytics**;
**animated-GIF export** of the wipe (server-side encoding); **npm package / framework
wrappers** (a publish pipeline vs. the self-contained file we emit); auto-resolving
third-party page URLs (Dropbox/Flickr) to direct image URLs (needs external API calls).

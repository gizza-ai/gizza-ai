# background-color-detector — competitor analysis (2026-08-06)

One WebSearch scan ("detect background color of image online tool"). All notes are paraphrased
observations of each product's public capability description — no competitor copy, naming, or
branding is reused anywhere in this tool.

## Competitors reviewed

1. **Background-Color-Thief (open-source JS library, GitHub SodhanaLibrary)** — detects the
   background colour of an image in the browser. Its published approach is the corner/edge
   heuristic: sample the image border, group near-identical pixels, and return the most common
   border colour as the background. Library only — no parameters exposed as a product UI, no
   uniformity verdict, no text-contrast advice.
2. **Azure AI Vision — colour-scheme detection (Microsoft)** — an image-analysis API that returns
   the dominant *background* colour, the dominant *foreground* colour, a small set of dominant
   image colours, an accent colour (a saturated, "vibrant" representative), and a
   black-and-white flag. Colours come back as coarse names from a fixed 12-name vocabulary plus a
   hex accent value. No knobs: no sampling region, tolerance, or coverage threshold.
3. **Colour-picker-from-image style web tools (e.g. the QuickColor / Color Picker From Image
   class of free single-page tools)** — upload an image, click any pixel to read its HEX/RGB, and
   get an auto-generated palette of prominent colours (default around 8 swatches, adjustable
   roughly 3–24). Table stakes there are: multiple colour notations for the picked value, a
   copyable hex, and a palette side-panel. Background detection itself is manual — the user is
   expected to click a corner.

## Table stakes → our decision

| Capability (paraphrased) | Seen in | Decision |
| --- | --- | --- |
| Border/corner sampling as the background heuristic | 1, and the standard MATLAB/CV answer | **In model** — `region` enum (`border`/`corners`/`edges`/`full`), default `border` |
| Adjustable sampled border thickness | implied by 1 (hard-coded there) | **In model** — `border_percent`, default 10 % of the shorter side |
| Grouping near-identical pixels before ranking | 1 | **In model** — quantised histogram bucket, then the exact mean of the winning bucket |
| Match tolerance for "same colour" | implied | **In model** — `tolerance` (0–100 % of full channel range, default 6) |
| Verdict: is there actually a solid background? | none of the three | **In model, our differentiator** — `is_uniform` + `coverage_percent` + `uniform_threshold` param + `confidence` |
| Transparent-background detection (PNG alpha) | none | **In model** — `ignore_transparency`, `is_transparent`, `transparent_percent` |
| Multiple colour notations (hex / rgb / rgba / hsl) | 3 | **In model** — all four returned, plus `#rrggbbaa` |
| Second-most-common background candidate | 3 (as a palette) | **In model** — `second_color` + its coverage, which is what exposes gradients |
| Per-corner readout so a user can sanity-check | 1 (corner-based) | **In model** — four corner hexes + `corners_agree` |
| Black-and-white / dark-vs-light flag | 2 | **In model** — relative luminance, `is_dark` |
| Suggested readable text colour + WCAG contrast | none (an obvious follow-on) | **In model** — `suggested_text_color`, `contrast_ratio` |
| Coarse colour *names* from a fixed vocabulary | 2 | **In model, cheap** — a nearest-name label (`color_name`) from a small CSS-ish set |
| Dominant *foreground* colour / accent colour | 2 | **Out of model for this tool** — foreground/accent extraction is the existing palette tool's job (a separate block already extracts top-N dominant colours with coverage); duplicating it here would fork one capability across two tools |
| Click-a-pixel picking, live preview canvas, palette swatch UI | 3 | **Out of model here** — a per-pixel read is already a separate block; this tool is a URL/ref → JSON analyser with no standalone page |
| Actually removing/replacing the background | (adjacent products) | **Out of model** — needs segmentation/matting; the repo's background-replace block covers the solid-colour case |

## UX patterns noted

Competitor UIs lean on swatch chips, a copy-to-clipboard hex, and a dark/light preview. This block
is a chat + CLI JSON analyser (image URL/ref in, report out) with **no standalone page** — the same
shape as the other image→JSON analysers in this repo (average-colour, palette-extract, tilt-checker)
— so those visual affordances land as *data* instead: every notation is returned pre-formatted for
copy/paste, `suggested_text_color` + `contrast_ratio` stand in for the dark/light preview, and the
per-corner hexes stand in for the swatch strip. A human-readable `note` summarises the verdict in one
sentence so the chat surface has something to say without post-processing.

## Not a duplicate of an existing block

- *average-colour* returns the mean of the **whole** image; a photo's mean is never the backdrop.
- *palette-extract* ranks the top-N **globally dominant** colours; a large subject outranks the
  backdrop, and it gives no uniform/gradient verdict.
- *colour-picker* reads **one caller-chosen pixel**; it doesn't decide which pixel to read.
- *trim* auto-crops a uniform border and outputs an **image**; it detects a border colour internally
  but reports crop geometry, not a background report.

This tool answers a question none of them answer: *what is the backdrop, and is it actually a
solid fill?*

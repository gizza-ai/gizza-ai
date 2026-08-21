# color-frequency-table — competitor analysis (2026-08-21)

Scan run **before** implementing, per `/create-next-tool` step 4. Everything below is a
paraphrase of observed behaviour; no competitor copy, branding or trademark is reproduced.

Backlog row: `color-frequency-table` — "Lists the top-N exact pixel colors with their counts and
percentage coverage of the image." (type hint: pure)

## Duplicate check (before building)

`ls blocks/ | grep -iE 'color|palette|histogram|pixel|frequency'` surfaces 29 neighbours. The four
that could plausibly overlap were read:

| Existing block | What it does | Why this tool is distinct |
| --- | --- | --- |
| `color-palette-extract` | NeuQuant derives an *optimal N-entry palette*, then re-maps every pixel to the nearest palette entry and reports share. Only param: `colors` 1-64. | The reported hexes are palette centroids, not colours that necessarily occur in the image. This tool counts the **exact** RGBA values present, so a 4-colour logo reports exactly those 4 hexes and a total unique-colour census. |
| `image-color-quantize` | Rewrites the image to ≤N colours, returns PNG bytes. | Image transform, not a report. |
| `image-histogram-analyzer` | Per-channel 256-level R/G/B/luma histograms + exposure verdict. | Marginal per-channel distributions; it can never tell you which *combined* RGB triples exist or how often. |
| `image-average-color` / `background-color-detector` | One mean colour / one backdrop colour. | Single-colour answers, not a frequency table. |

Verdict: **not a duplicate** — no existing block reports exact-colour counts or a unique-colour
count. Building it.

## Competitors surveyed (top 3 + 2 skimmed)

1. **PlanetCalc "Image color set"** — upload → table of colours sorted by frequency, most common
   first; reports the total colour count and the prevailing colour; exposes a decimal-precision
   setting and a range selector limiting how many per-colour preview images are rendered
   (rendering those is the expensive part, so it defaults to the first 10). No tolerance/grouping
   controls. States that large images slow the browser down.
2. **ImageOnline.io "Pixel Counter"** — JPG/PNG/WebP, browser-side. Reports dimensions, total
   pixels, megapixels, **unique colour count**, and the top colours with percentage bars; prints a
   *complete* inventory (hex + count + percent) when the image has ≤48 colours. Has a
   **tolerance slider** (0 = exact RGB match, higher = neighbouring shades by RGB distance) used
   for its click-a-colour counting mode. Analyses up to **4 MP** pixel-by-pixel and above that
   falls back to a 4 MP nearest-neighbour sample with results *explicitly marked as estimated*.
   One button copies the whole report as plain text.
3. **Online PNG Tools "Find PNG color count"** — toggles for total colour count vs unique colour
   count; separate opaque / transparent colour lists each with their own "how many to print"
   count; a colour-notation selector (hex, rgb(a), hsl, hsv, hsi, lab, lch, hcl); and per-colour
   toggles for printing the pixel count and the usage percentage. Also breaks the census down by
   grayscale / transparent / translucent / opaque.
4. *(skimmed)* **ToolBird / Toolin pixel counters** — click-a-colour + tolerance, per-colour count
   and percentage, distribution chart, fully client-side.
5. *(skimmed)* **Novaboard image colour counter** — unique colour count *including alpha*, full
   breakdown sorted by usage.

## Table stakes → decision

| Capability | Seen at | Decision |
| --- | --- | --- |
| Top-N colours with hex, pixel count, percentage, sorted most-common-first | 1,2,3,4,5 | **In** — `top` (1-256, default 10); every row carries rank/hex/rgb/count/percent. |
| Total unique colour count | 2,3,5 | **In** — `unique_colors`, plus `listed_percent` / `remaining_colors` / `remaining_percent` so the tail is never silently dropped. |
| Dimensions / total pixels / megapixels | 2 | **In** — `width`, `height`, `total_pixels`, `megapixels`. |
| Grayscale / opaque / translucent / transparent breakdown | 3 | **In** — `grayscale_unique_colors`, `translucent_unique_colors`, `transparent_pixels`, `transparent_percent`. |
| Colour notation choice (hex / rgb / rgba / hsl) | 3 | **In** — `color_format` picks the column used in the rendered table+CSV; the JSON rows always carry all four notations, so nothing is lost. |
| Tolerance / grouping of near-identical shades | 2,4 | **In** — `quantize` (1-64, default 1 = exact). Buckets each channel and reports the **mean** of the pixels in each bucket, so a JPEG-noisy sky collapses to one row with a real average colour instead of thousands of near-dups. |
| Minimum share filter | implied by 1,2's "print first N" | **In** — `min_percent` (0-100, default 0). |
| Alpha handling / count alpha as part of colour identity | 3,5 | **In** — `ignore_transparency` (default true) excludes near-transparent pixels from the census and reports them separately; alpha is part of the colour key otherwise (`hex_rgba` per row). |
| 4 MP analysis cap with estimates clearly marked | 2 | **In** — exact below 4 MP; above it a symmetric row/column stride sample sets `sampled=true`, `stride`, and a warning naming the estimate. |
| Copy the whole report in one action | 2 | **In** — the response carries a ready-made aligned `table` string and a `csv` string. |
| Alternate sort orders (by luminance / hue) | not offered by any of the three | **In** anyway — `sort` = frequency \| luminance \| hue; the top-N selection always stays by frequency, only the presentation order changes, so "top 10" keeps meaning "the 10 most common". |
| Plain-English colour name per row | none of the three | **In** — small vocabulary, same one `background-color-detector` uses; makes the chat answer readable. |
| LAB / LCH / HCL / HSV / HSI notations | 3 | **Out (scope)** — hex/rgb/rgba/hsl cover the CSS-usable set; the other five are colour-science notations that would need a full CIE conversion stack for a column almost nobody copies. Listed, not built. |
| Per-colour preview image ("only these pixels") | 1 | **Out of model** — this block returns a JSON report; image output would need a second, image-returning block (`image-color-isolate`-shaped). |
| Click a colour in the image to count it | 2,4 | **Out of model** — interactive canvas UI. Also note there is **no page surface** for this tool: the page generator's file-upload runtime is ffmpeg-only, so pure-Rust image analysers ship chat + CLI only (same as `image-histogram-analyzer`, `image-average-color`, `background-color-detector`). |
| Distribution bar chart | 2,4 | **Out (scope)** — `histogram-chart` already renders charts; this block stays a report. |
| Pixel-art scale detection | 2 | **Out (scope)** — a separate heuristic tool, unrelated to the colour census. |

## Surfaces

- **Chat / CLI**: both — `gizza tool color-frequency-table url=… top=…`.
- **Page**: none (platform limitation above). Stated here rather than silently skipped.

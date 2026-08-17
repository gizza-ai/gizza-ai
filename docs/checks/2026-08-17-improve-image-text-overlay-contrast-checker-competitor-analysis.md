# image-text-overlay-contrast-checker competitor analysis — 2026-08-17

## Scope

Tool: `image-text-overlay-contrast-checker` — scan a photo for the worst-case region where
overlaid text of a given colour would fail WCAG contrast, and say what would fix it.

The question the tool has to answer is spatial: *"where on this hero image would white caption
text be hard to read?"* A plain two-colour checker cannot answer it, because a photo has no single
background colour.

## Competitor scan

One search for the tool's function ("check text contrast over image WCAG tool overlay text on
photo readability checker"), then the top three real tools were skimmed. Everything below is
paraphrased from observed behaviour; no competitor copy, branding or trademark is reproduced here
or in the tool.

1. **Image accessibility / text-on-image checker (drag-the-text model).** Upload a JPEG or PNG
   (roughly 25 MB ceiling), drop a text element onto the picture, and drag it around. Text colour
   comes from a picker; a small/large text toggle switches the threshold. Reports a live contrast
   ratio with a qualitative word attached, plus separate AA and AAA pass/fail badges. States the
   familiar thresholds (4.5:1 small text, 3:1 large text, 3:1 graphics). Everything is manual: the
   user has to find the bad spot themselves by dragging.
2. **Image colour contrast checker (upload → pick colours → verdict).** Upload an image, pick a
   text colour and a background colour, get a ratio from 1:1 to 21:1 with AA (4.5:1) and AAA (7:1)
   verdicts split by normal vs large text. Scoped explicitly as a single-pair check, not an audit.
   Also manual: the background colour is whatever the user samples.
3. **General WCAG colour contrast checker (no image input).** Hex or RGB in (with per-channel
   sliders), live sample-text preview, ratio plus AA/AAA verdicts for normal and large text and a
   3:1 UI-component verdict, and a FAQ covering AA vs AAA, graphics/UI thresholds, colour blindness
   and remediation strategy. This is the vocabulary users already expect; it has no notion of an
   image.

Common remediation advice across all three (and the surrounding accessibility literature): when a
photo is too busy, put a semi-transparent dark or light scrim between the picture and the text.
None of the three computes how much scrim is needed.

## Table-stakes matrix

| Capability / UX pattern | Decision | Notes |
| --- | --- | --- |
| Image input (upload / URL) | In model | `Input::Image` — `url` ⊕ `ref`; PNG, JPEG, WebP, GIF, BMP, TIFF. |
| Text colour in hex / rgb / hsl / CSS name | In model | `text_color`, default `#ffffff`; parsing shared with `color-contrast-checker`. |
| Contrast ratio 1:1 – 21:1 | In model | WCAG 2.x relative luminance, reused from the same core so the two tools never disagree. |
| AA vs AAA | In model | `level` enum. |
| Normal vs large text thresholds | In model | `text_size = normal \| large`, 4.5/3 at AA and 7/4.5 at AAA. |
| UI-component / graphics 3:1 threshold | In model | `text_size = ui` (SC 1.4.11 has no AAA variant — documented, stays 3:1). |
| Pass/fail verdict | In model | `passes`, plus per-window `passes` and a leading PASS/FAIL in `note`. |
| Find the worst spot **for** the user | In model — the differentiator | A text-block-shaped window slides over the picture; every position is scored. Competitors make the user drag until they find it. |
| Where the caption *can* go | In model — the differentiator | Twelve candidate areas (three full-width bands + the nine thirds cells), each with its own worst window, ranked best-first. |
| How much scrim would fix it | In model — the differentiator | Minimum black and white overlay opacity that lifts *every* window over the bar, with a paste-ready `rgba()` value. |
| Would black/white text be better? | In model | `alternatives` scores pure black and pure white over the same windows. |
| Caption-shaped sampling, not per-pixel | In model | `window_width` / `window_height` as percentages, stepping a quarter of their own size. |
| Scan only the band the caption lives in | In model | `region = full \| top \| middle \| bottom \| left \| center \| right`. |
| Transparency handling | In model | `alpha_background = white \| black` — what shows through a PNG with alpha. |
| Heat-map data | In model (data, not pixels) | `output = full` returns the per-window ratio grid; `csv` returns it as a table. Capped at 10 000 windows. |
| Interactive drag-the-text canvas | Out of model | Needs a live canvas UI; this block is chat + CLI (see below). |
| Eyedropper / manual colour sampling | Out of model | Replaced by measuring every window automatically, which is strictly more informative. |
| Suggesting a *nearer* accessible text colour | Considered, rejected | `color-contrast-checker` already does hue-preserving suggestion for a known background pair; over a photo the honest answers are the scrim opacity and the black/white comparison, both of which ship. |
| Full-page accessibility audit | Out of model | Different tool class; needs a DOM, not an image. |
| Cloud batch / accounts / API keys | Out of model | Browser-local, no-account, no-server. |

## Surface decision (why there is no page)

Image-input, text-report tools in this repo ship as **chat + CLI with no standalone page** —
`image-histogram-analyzer`, `image-average-color`, `background-color-detector` and
`image-blank-detector` all follow it. The reason is structural, not cosmetic: the page generator
only wires a file input for `runtime = "ffmpeg"` and `runtime = "model"`
(`tools/generator/assets/runtime/tool.js`); a `wasm`-runtime page has no path for handing uploaded
bytes to a wasm decoder. A page here would render an upload control that never reaches the block.
Consequently there is also **no Playwright page spec** — the verifiable surfaces are the
descriptor/schema (drift-guard unit test, which is what the chat surface consumes) and the CLI.

## Descriptor decisions

- Image source: `url` ⊕ `ref` (`Input::Image`), 32 MiB input cap.
- Fixed choices as `Param::enumv`: `level`, `text_size`, `region`, `alpha_background`, `output`.
- Numeric: `window_width`, `window_height` (percent, 1–100, defaults 30 / 10).
- Free text: `text_color` (hex / rgb / hsl / CSS name, default `#ffffff`).
- Every parameter carries a `.describe()` stating the accepted values, the unit, the default and
  why it matters.

## Stated limits (in the descriptor + errors, not just in code)

- Decoding budget ≈ 44 MB for input + decoded raster; over that the error names the dimensions and
  tells the user to re-export smaller rather than trapping the sandbox.
- The scan runs on a box-downscaled copy, at most 512 px on the long edge — exact for text-sized
  area averages, which is what WCAG contrast over a block of text actually depends on.
- `output = full` / `csv` are capped at 10 000 windows; the error says which knob to raise.
- Analyser only — the image is never modified.

## Verification focus

- Unit: white text over a half-black/half-white image (worst 1:1, best 21:1, top band safe,
  bottom band not); region-only scan; large-text threshold switch; transparency composited over
  white vs black; grid/CSV shape; minimum-scrim opacity is genuinely minimal (one step less fails).
- Error paths: empty bytes, non-image bytes, out-of-range window percentages, every enum's bad
  value, and the grid cap.
- Drift-guard: authored chat schema compared against `schema_json()`.
- CLI: one exact-output case against a public image URL.

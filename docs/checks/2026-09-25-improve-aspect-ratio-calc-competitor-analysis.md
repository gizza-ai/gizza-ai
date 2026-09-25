# aspect-ratio-calc — competitor analysis (2026-09-25)

Scan run **before** implementing, per `/create-next-tool` step 4. All competitor notes are
**paraphrased observations of behaviour**; no competitor copy, branding, or trademarks were
reproduced. "Out-of-model" items are listed, never built.

## Duplicate check (done first)

`ls blocks/ | grep -i -E 'aspect|ratio|resol'` surfaces `aspect-ratio-validator`,
`video-aspect-pad`, `video-aspect-ratio-fix` and `image-info`. Read
`blocks/aspect-ratio-validator/core/src/lib.rs` + `src/lib.rs`:

- Its `width` and `height` are both `Param::number(...).required()` with `min(1.0)`, and
  `check_dimension` rejects anything `<= 0`. It **cannot** solve for an unknown dimension — the
  headline job on this row ("compute missing width/height to hit a target aspect ratio").
- Its verdict shape is validation: `status` PASS/FAIL/INFO, `tolerance_percent`, `deviation_percent`,
  `reason` = ok/too_wide/too_tall, plus crop/pad suggestions for an asset that already exists.
- The overlap is the report-only half (GCD-reduced ratio + nearest standard). That half is the
  *second* clause of the row; the first clause is a genuinely absent capability, and a
  solve-for-the-unknown calculator with no PASS/FAIL verdict is a different tool shape (and a
  different user intent) from a spec checker. `image-aspect-ratio-detector` was skiplisted against
  the validator because that row was *report-only*; this row is not.

Built, not skiplisted.

## Competitors reviewed

| # | Tool | Shape | What it does |
|---|------|-------|--------------|
| 1 | calculatorsoup — aspect ratio calculator | 4-mode calculator | Width + height fields with a unit selector (none / px / in / cm / mm). Four modes: derive the ratio, solve width from height, solve height from width, and a compare/resize view. Prints the decimal ratio, the **nearest standard** ratio, and the **diagonal**. Documents both reduction methods (divide for `q:1`, GCD for whole-number `w:h`) and ships two 30-row standard-ratio tables. Worked example: 1920×1080 → 16:9. |
| 2 | calculateaspectratio.com | solve-the-missing-dimension | Separate ratio-width / ratio-height fields plus pixel width / pixel height. Six ratio presets (16:9, 9:16, 21:9, 4:3, 3:2, 1:1) plus custom. Typing either pixel dimension instantly fills the other. Draws a live proportion box labelled with both forms (`16:9 (1.78:1)`). Content covers platform picks (vertical vs widescreen), ratio-vs-resolution, and crop-vs-pad. |
| 3 | aspectratiocalculator.com | two calculators on one page | A "basic" calculator (W×H → ratio) and a "resize" calculator (pick a ratio, enter one dimension, get the other). Nine presets: 16:9, 4:3, 1:1, 9:16, 21:9, 3:2, 5:4, 2:1, 32:9, plus custom. Outputs the `x:y` ratio, the dimensions, **portrait/landscape**, and the **total pixel count**. Heavy platform-guide cross-linking and sibling PPI/DPI tools. |
| 4 | Digital Rebellion — aspect calculator | broadcast/post preset picker | Ratio field + width/height fields, but the emphasis is a 30+ entry **format preset list** (NTSC/PAL SD variants, 720/1080 HD and HDV, 2K/4K Academy, Super 35, full aperture, anamorphic scope, a RED sensor mode) and an 11-entry ratio library from 1.33:1 to 2.39:1. Solves resolution from ratio or ratio from resolution. Emits a shareable link to the calculation. No pixel-aspect/anamorphic squeeze control despite listing scope formats. |
| 5 | imagy — aspect ratio calculator | solve-the-missing-dimension | Width/height plus ratio-width/ratio-height. Three explicit modes: width from ratio+height, height from ratio+width, ratio from width+height. Pre-seeded with 1920×1080. Prints the solved pixel value and the simplified ratio in slash form (`16/9`). FAQ covers changing an image's ratio, the common ratio families, video-format compatibility, and how to do the arithmetic by hand. No stated limits. |

(codeshack.io's calculator was in the search results but returns HTTP 403 to a plain fetch, so it
was replaced by imagy per the "don't run with 4" rule.)

## Table stakes → decisions

| Capability seen | Where it lands |
|---|---|
| Solve the missing width or height from a ratio + one dimension (1, 2, 3, 4, 5) | **In model** — the headline job. `ratio` + exactly one of `width`/`height`; the other is computed. |
| Simplify a resolution to its ratio (1, 3, 5) | **In model** — leave `ratio` blank and pass both dimensions; GCD reduction, with a `w:1` decimal fallback for non-integer inputs. |
| Normalise a ratio on its own, no pixels (4) | **In model** — `ratio` alone (e.g. `1920x1080` → `16:9`) reports the ratio, its decimal form, the nearest standard and the CSS forms with no pixel maths. |
| Ratio written many ways (`16:9`, `16/9`, `1.85:1`, `1920x1080`, bare decimal) | **In model** — one permissive parser, so the *resize-preserving-the-original-ratio* case is just `ratio=1920x1080, width=1280` and needs no fourth field. |
| Ratio presets / preset chips (2, 3, 4) | **In model** — `[[example]]` chips for 16:9, 9:16, 4:3, 21:9, 1:1 and a cinema scope case; the ratio field itself stays free-text so any of Digital Rebellion's 30 formats is typeable. |
| Nearest standard ratio with a human name (1) | **In model** — a 25-row standard table (square → 32:9, plus the portrait mirrors and ISO paper) with the deviation in percent. |
| Portrait / landscape / square (3) | **In model** — `orientation` in every report. |
| Total pixel count (3) | **In model** — `total_pixels` plus a megapixel figure. |
| Diagonal (1) | **In model** — `diagonal`, in the same unit as the inputs. |
| Decimal `q:1` form alongside `w:h` (1, 2) | **In model** — `ratio_decimal` and `ratio_x_to_1` are both reported. |
| Whole-pixel results (every competitor rounds) | **In model** — `rounding` = `nearest` (default) / `up` / `down` / `even` / `exact`. `even` is the H.264/H.265 requirement no competitor exposes; `exact` keeps the fraction for print work. |
| Shareable link to a calculation (4) | Covered by the platform — every gizza page pre-fills and auto-runs from `?ratio=…&width=…`. |
| Live proportion box (2) | **Considered, rejected** — see below. |
| Unit selector: px / in / cm / mm (1) | **Considered, rejected** — a ratio is unit-free; see below. |
| Machine-readable output | **In model** — `output_format` = `summary` (default) / `dimensions` / `ratio` / `decimal` / `css` / `json`. No competitor offers a scriptable form, and it is the natural chat/CLI shape. |
| CSS `aspect-ratio` + legacy padding-top percentage | **In model** — `output_format = css` emits both. This is the one thing the developer-facing competitor (codeshack) is known for and none of the reachable five ship it. |

## Considered, not built

- **Live visual proportion box** (2) — the page renders a text/`format = "text"` output surface; a
  bespoke canvas would mean per-tool JS in the shared runtime, which the platform rule forbids.
  The worked examples and the orientation/megapixel lines carry the same information in text.
- **Unit selector (in / cm / mm)** (1) — a ratio is unit-free and the solved dimension always comes
  back in whatever unit went in, so a selector would be a label with no maths behind it. The page
  states this instead. The one thing a unit selector buys — a diagonal in inches — needs a DPI
  input and belongs in a pixel-density tool, not here.
- **A 30-row broadcast format preset menu** (4) — SEO/lookup filler: every one of those formats is
  typeable into the ratio field as `WxH`. Six preset chips cover the real traffic; the page states
  that any `WxH` works.
- **Image/video upload to read the dimensions automatically** — out of scope for a pure-arithmetic
  block; `blocks/image-info` already reports an image's dimensions and `blocks/media-info` a
  video's, and the page copy points at that hand-off.
- **PASS/FAIL validation against a target with a tolerance, plus crop/pad boxes** — deliberately
  *not* rebuilt: `blocks/aspect-ratio-validator` owns that job, and the page copy links the two
  intents so neither tool grows the other's schema.
- **Pixel-aspect-ratio / anamorphic squeeze** (4 lists scope formats but does not compute the
  squeeze) — a genuinely different model (storage vs display aspect); noted as a possible sibling,
  not forced into this schema.

## UX patterns adopted

- Smart defaults: the page loads pre-filled with `ratio = 16:9` and `width = 1920`, so a result is
  on screen before the user types (imagy's pre-seeded 1920×1080, done declaratively).
- Preset chips (`[[example]]`) for the six scenarios the competitors all use as their examples.
- Friendly `<select>` labels via `[input.labels]` for `rounding` and `output_format`.
- Real placeholders on every text/number field; the limits (max dimension 1 000 000, what happens
  when both dimensions and a ratio are supplied) stated on the page, not only in error strings.
- Errors name the missing input — "give a ratio, or both width and height" — rather than a bare
  "invalid input".

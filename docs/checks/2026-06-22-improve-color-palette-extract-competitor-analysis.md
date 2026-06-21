# color-palette-extract — competitor analysis (2026-06-22)

Tool: `blocks/color-palette-extract` — extract the dominant color palette from an
image as hex codes (image url/ref → JSON). Pure-Rust (`image` + `color_quant`
NeuQuant), runs on all backends incl. the chat Service Worker. Surfaces: chat +
CLI. No standalone page (image-input → text report, the F3 no-page pattern, like
`image-info` / `image-color-picker`).

## Top competitors surveyed

1. **Coolors — Image Picker** (coolors.co/image-picker) — extracts a palette from
   an uploaded photo; hex/rgb/hsl per swatch; export to many formats; accessibility
   checks; designed around a 5-color default with adjustable count.
2. **Adobe Color — Create from Image** (color.adobe.com/create/image) — auto-detects
   dominant + accent colors; outputs HEX/RGB/HSB/LAB; "color mood" selector
   (colorful / bright / muted / deep / dark).
3. **imagecolorpalette.com** — shows 6–12 colors instantly, each with HEX/RGB/HSL;
   download as PNG swatch sheet, copy as CSS variables, or grab JSON.
4. **imagetoolo Image Palette Generator** (imagetoolo.com) — up to 50 dominant
   colors; HEX/RGB/HSL plus CSS variables, SCSS, SVG, Tailwind config export;
   100% browser-side.
5. **Decoratly — Color Palette From Photo** (decoratly.com) — reads every pixel,
   identifies the 8 most dominant colors; local/offline; copy hex codes.

(Also reviewed: Canva palette generator, Figma color picker, codeshack, ImageMagixOnline
dominant-color finder — same feature shape.)

## Capability diff (competitor feature → our status)

| Feature                                   | Competitors        | color-palette-extract |
|-------------------------------------------|--------------------|-----------------------|
| Dominant palette derived from the image   | all                | yes (NeuQuant)        |
| HEX `#rrggbb` per color                    | all                | yes                   |
| RGB per color                              | all                | yes (`rgb()` + r/g/b) |
| **HSL per color**                          | all                | **added** (`hsl()` + h/s/l) |
| Dominance ordering (most-used first)       | most               | yes (by pixel share)  |
| Per-color share / percentage               | some               | yes (`fraction` + `percent`) |
| Adjustable number of colors                | all (5/6/8/12/50)  | yes (`colors`, 1–64, default 6) |
| Transparent-background handling            | varies             | yes (ignores alpha < 16) |
| **Copy-as-CSS-variables**                  | imagecolorpalette, imagetoolo | **added** (`css_variables`) |
| Plain list of hex codes (quick copy)       | implicit           | yes (`hex[]`)         |
| Image dimensions reported                  | rare               | yes                   |

## Gaps closed this pass (in-model, pure compute)

- **HSL output** — every major competitor shows HSL alongside HEX/RGB. Added a
  hand-rolled `rgb_to_hsl` (no new dep) → `hsl()` string plus numeric `h`/`s`/`l`
  per swatch. Verified against known vectors (red/green/blue/black/white/gray) in a
  unit test.
- **CSS custom-properties export** — added a ready-to-paste `css_variables` block
  (`:root { --color-1: #...; }`) so the result drops straight into a stylesheet,
  matching imagecolorpalette / imagetoolo.

## Out-of-model / deliberately not built

- **PNG swatch-sheet / SVG / image export of the palette** — gizza's image-bytes
  output path exists (`build_media_envelope`), but this tool's value is the
  structured text report (hex/rgb/hsl/CSS) the LLM and CLI consume; a separate
  swatch-image render would be a distinct image-output tool, not a fit for this
  text-report block.
- **SCSS / Tailwind-config export** — niche format variants; the CSS-variables block
  plus the raw hex list already cover the copy-paste use cases, and the LLM can
  trivially reshape the returned hex array into any other format on request.
- **Accessibility / contrast checks, "color mood" presets, HSB/LAB color spaces** —
  separate concerns; LAB/HSB add a color-science dependency for marginal benefit
  over HEX/RGB/HSL.
- **No standalone page** — image-input → text report; the page file-input path is
  ffmpeg-only in this codebase, so image-report tools are chat + CLI only (same as
  `image-info`).

## Branding / IP

No competitor copy, branding, trademarks, or wording were copied. The description,
field names, and CSS-variable formatting are original.

## Verification (this pass)

- `cargo test` core: 7/7 pass (dominance ordering, hex format, count clamp, fraction
  sum, transparent-pixel exclusion, garbage rejection).
- `cargo test` block: 2/2 pass (chat-schema drift guard + HSL known-vector test).
- `wafer build`: `OK gizza-ai/color-palette-extract v0.1.0` — block.wasm validates +
  instantiates (network-aware).
- CLI: `gizza tool color-palette-extract url=… colors=4` → correct palette with
  hex/rgb/hsl/h/s/l/percent + `css_variables`; red/green image → exactly `#ff0000`
  (50.9%) + `#00ff00` (49.0%); bad-image URL → graceful error.

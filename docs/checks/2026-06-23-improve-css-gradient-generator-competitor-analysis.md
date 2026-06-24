# css-gradient-generator — competitor analysis (2026-06-23)

Tool: `blocks/css-gradient-generator`. Input: color stops (+ optional positions),
gradient type, angle, radial shape, repeating flag, color-interpolation space.
Output: a ready-to-paste `background-image: <gradient>;` CSS declaration. Pure
compute (no I/O); verified on all three surfaces — chat block (`wafer build`),
CLI (`gizza tool`), and standalone page (Playwright).

## Top competitors surveyed

1. **cssgradient.io** — the canonical CSS Gradient maker. Linear/radial, angle
   slider, color stops, copy CSS. Strong picker UX; limited type coverage (no
   conic in the headline editor).
2. **css-gradient.com** — Linear, Radial, **Repeating**, **Conic**, and Text
   gradients. Cross-browser output. Broadest type coverage of the classic tools.
3. **hidekazu-konishi.com gradient tool** — linear/radial/conic with full control
   over angle, shape, size, **center position**, interactive stop editor, live
   preview, and **modern color-space interpolation (oklch / oklab / hsl /
   srgb-linear)** plus animated-gradient CSS.
4. **learnui.design gradient generator** — linear/radial/conic, one-click **CSS
   or SVG export**, uses an **LCH/perceptual** color model for smoother blends.
5. **muxgen / miromiro** — linear/radial/conic (miromiro adds **mesh**); palette
   moods (pastel/neon/…), and Tailwind/SCSS output in addition to CSS.

Sources:
- https://cssgradient.io/
- https://www.css-gradient.com/
- https://hidekazu-konishi.com/tools/css_gradient_generator_tool.html
- https://www.learnui.design/tools/gradient-generator.html
- https://www.muxgen.com/image-tools/gradient-generator
- https://miromiro.app/tools/gradient-generator

## Gap analysis (fit-to-model: text-in → CSS-string-out)

| Capability | Competitors | Our tool | Action |
| --- | --- | --- | --- |
| Linear gradients + angle | all | yes | in model — covered |
| Radial gradients + shape (circle/ellipse) | most | yes | covered |
| Conic gradients + start angle | css-gradient.com, hidekazu, learnui | yes | covered |
| Repeating gradients | css-gradient.com | yes | covered |
| Multiple color stops with explicit positions | all | yes (`#f00 25%`, bare-num %, px) | covered |
| Auto even-spacing of stops | all | yes | covered |
| rgb()/hsl()/named/hex+alpha colors | all | yes (paren-aware split, hex validation) | covered |
| **Modern color-space interpolation (oklch/oklab/lch/srgb-linear, + hue method)** | hidekazu, learnui | **ADDED this pass** (`interpolation` param → `in <space>` clause, polar-hue methods) | **closed** |

## Closed this pass

- **Color interpolation method** (`interpolation` param): emits the CSS Color-4
  `in <space>` clause (e.g. `linear-gradient(in oklch 90deg, …)`) for perceptually
  smoother blends, matching the modern-color-space feature in hidekazu/learnui.
  Validates the space against the CSS named set, and for polar spaces
  (hsl/hwb/lch/oklch) accepts a hue-rotation method (`shorter|longer|increasing|
  decreasing`). Added to core + chat descriptor + web + page (with unit tests, a
  CLI check, and a Playwright assertion). Drift-guard schema regenerated.

## Out of model (NOT built — UI/asset features, not a text→CSS transform)

- **Live visual preview swatch / interactive color-picker / draggable stops** —
  these are page-UI affordances; our page renders the CSS string output. The
  string is the supported deliverable, copy-pasteable into any preview.
- **SVG / PNG export** of the gradient as an image — a separate render pipeline
  (image-bytes output), out of scope for a CSS-text generator; would be its own
  block.
- **Tailwind / SCSS output variants** — a formatting layer; the standard CSS
  declaration is the portable baseline. Could be a future enhancement but is not a
  capability gap (the gradient value is identical).
- **Mesh gradients** (miromiro) — not a single standardized CSS function across
  browsers (requires multiple layered radial gradients or a paint worklet);
  deferred as not-yet-interoperable.
- **Animated-gradient CSS** (keyframes) — a separate animation concern, distinct
  from the gradient value itself.
- **Curated palette moods (pastel/neon/…)** — a color-suggestion feature better
  served by the existing `color-palette-generator` block.

## Verification

- `cargo test --workspace`: 23 core unit tests + 1 chat-schema drift test pass.
- `wafer build`: chat block validates and instantiates (330 KiB).
- CLI: linear/radial/conic/repeating/interpolation all produce correct CSS; error
  cases (too few stops, bad type/shape/hex/interpolation) exit non-zero with a
  clear message.
- Playwright (`tool-page-css-gradient-generator.spec.ts`): 4/4 pass — linear,
  radial circle, oklch interpolation, repeating conic.

No competitor copy, branding, or trademarks were reproduced.

# css-px-to-rem — competitor analysis (2026-08-23)

Scan run **before** implementation, per `/create-next-tool` step 4. All competitor material below
is **paraphrased** from public docs/tool UIs — no copy, branding, or trademarks reproduced.

Backlog row: `css-px-to-rem` — *Converts px length values in CSS to rem against a configurable root
font size.* Type: **pure**.

Important framing: the backlog description is a **CSS-source rewriter** (stylesheet in → stylesheet
out), not a single-number calculator. Most consumer "px to rem" pages are calculators; the closest
functional competitor is the PostCSS plugin family. Both classes were scanned, because the
calculators define the table-stakes *options* (root size, precision, presets) and the plugin defines
the table-stakes *rewriting semantics* (property filtering, min value, media queries).

## Competitors scanned

| # | Competitor | Class | Reachable |
|---|---|---|---|
| 1 | postcss-pxtorem (README, GitHub raw) | build-time CSS rewriter | yes |
| 2 | nekocalc px↔rem converter | single-value calculator | yes |
| 3 | miniwebtool px↔rem converter | batch value calculator | yes |
| 4 | cssunitconverter px→rem | single-value calculator + preview | yes |

`npmjs.com/package/postcss-pxtorem` returned HTTP 403 to the fetcher; replaced with the same
project's GitHub README (same content, reachable), so four real competitors were reviewed rather
than running with fewer.

### 1. postcss-pxtorem (build-time plugin)

Options and defaults (paraphrased from its README):

- `rootValue` — root font size, default 16 (may also be a function of the file).
- `unitPrecision` — decimal places kept on the rem result, default 5.
- `propList` — which CSS properties get converted; default is the typography set
  (`font`, `font-size`, `line-height`, `letter-spacing`, `word-spacing`). Wildcard syntax:
  `*` = all, `*part*` = contains, `part*` prefix, `*part` suffix, `!x` = exclude,
  so `['*', '!letter-spacing']` = everything except that one.
- `selectorBlackList` — selectors to leave alone entirely.
- `replace` — true replaces the px declaration; false keeps px and appends the rem one as a
  progressive-enhancement pair.
- `mediaQuery` — whether px inside media-query conditions is converted; default false.
- `minPixelValue` — values smaller than this are left in px (the standard hairline-border idiom).
- `exclude` — file-path filter (build-tool concept).
- Documented opt-out idiom: writing the unit with a capital (`1Px`) keeps a single value in px
  while staying valid CSS.

### 2. nekocalc

Two linked number fields (px and rem) with a swap control, an adjustable base defaulting to 16,
copy buttons on each field, ~2-decimal display, and static two-way lookup tables. Single values
only; no stylesheet input.

### 3. miniwebtool

Value box accepting up to ~20 space/comma-separated numbers (trailing `px` tolerated), root-size
preset chips (10/12/14/16/18/20, the 10 being the "62.5% trick"), four directions (px→rem, rem→px,
px→em, em→px), precision selector (auto-trim, or fixed 2/3/4), results as a table plus a
copy-ready CSS block that keeps the original px as comments, a true-scale ruler, and an
accessibility preview showing rescaling when the browser font size changes.

### 4. cssunitconverter

px and rem fields plus an editable base size, bidirectional live conversion, four-decimal lookup
table, and a live text preview rendered at the computed size. Also ships a browser extension.

## Table stakes → decisions

| # | Table stake (source) | In/out of model | Where it landed |
|---|---|---|---|
| 1 | Configurable root font size, default 16 (all four) | in-model | `root_font_size` param, default 16, min 1 |
| 2 | Decimal precision control (postcss `unitPrecision` 5; miniwebtool auto/2/3/4) | in-model | `precision` param, default 5, 0–10, trailing zeros always trimmed (that's the "auto" behavior) |
| 3 | Reverse direction rem→px (nekocalc swap, miniwebtool, cssunitconverter) | in-model | `direction` enum `px-to-rem` \| `rem-to-px` |
| 4 | Property allow/deny filtering with wildcards (postcss `propList`) | in-model | `properties` param, same `*`/`!` wildcard grammar, default `*` (a whole-stylesheet tool's users expect everything converted; the plugin's typography-only default suits build pipelines, not a paste box) |
| 5 | Minimum px value left alone — 1px borders (postcss `minPixelValue`) | in-model | `min_pixel_value` param, default 0 |
| 6 | Media-query conversion toggle (postcss `mediaQuery`, default off) | in-model | `media_queries` boolean, default false |
| 7 | Selector blacklist (postcss `selectorBlackList`) | in-model | `ignore_selectors` param, comma-separated substrings |
| 8 | Keep px as a fallback declaration (postcss `replace: false`; miniwebtool keeps px as comments) | in-model | `keep_fallback` boolean, default false — emits the original declaration then the converted one |
| 9 | Uppercase-unit opt-out idiom (`1Px`) (postcss) | in-model | falls out of matching the unit case-sensitively; documented on the page |
| 10 | 62.5%-trick root of 10 (miniwebtool preset) | in-model | `[[example]]` preset chip + FAQ entry |
| 11 | `0px` normalization | in-model | `unitless_zero` boolean, default true (`0px` → `0`, never `0rem`) |
| 12 | Never touch px inside `url()`, quoted strings, or comments (implicit in every rewriter) | in-model | tokenizer skips those spans; covered by unit + page tests |
| 13 | Preset root-size chips (miniwebtool, 10–20) | in-model | `[[example]]` chips |
| 14 | Static px↔rem lookup table (all three calculators) | in-model (as copy) | conversion table in `content.md` |
| 15 | Live rendered-size preview / true-scale ruler (cssunitconverter, miniwebtool) | **out of model** | the generic page renderer emits a text result; a bespoke live preview needs per-tool UI. Listed, not built. |
| 16 | Accessibility preview: rescaling at other browser font sizes (miniwebtool) | **out of model** | same reason; explained in FAQ copy instead |
| 17 | Browser extension / embeddable widget (cssunitconverter, miniwebtool) | **out of model** | distribution surface, not a tool capability |
| 18 | File-path `exclude` (postcss) | **out of model** | no filesystem in a browser-local tool |
| 19 | `rootValue` as a callback per file (postcss) | **out of model** | build-tool concept; no code execution in the schema |
| 20 | px→em / em→px directions (miniwebtool) | in-model but **considered, rejected** | `em` depends on the *parent* element's computed size, which a stylesheet rewriter cannot know; converting text px→em against the root would be silently wrong. Called out in the FAQ instead of shipped as a lying option. |

Every table stake above is either a descriptor param / documented behavior, or an explicitly listed
out-of-model (or reasoned-rejection) item — none dropped silently.

## Where this tool goes beyond the scanned set

- The calculators handle single values or short lists; this one rewrites a **whole stylesheet** and
  preserves comments, strings, `url()`, at-rules, and nesting.
- postcss-pxtorem is a build-step dependency; this runs locally in the browser and in a CLI with no
  install or config file.
- Both directions (px→rem and rem→px) in one rewriter — the plugin family needs a second plugin for
  the reverse.

## UX patterns adopted

- Preset chips for the common roots (16 default, 10 for the 62.5% trick) and for the 1px-border
  `min_pixel_value` idiom — the miniwebtool preset pattern, expressed with the generator's
  declarative `[[example]]` chips.
- Multiline paste field for the stylesheet, `<select>`s for the fixed choices, checkboxes for the
  toggles, real placeholders showing runnable input.
- Copy button and Reset come from the shared page chrome.

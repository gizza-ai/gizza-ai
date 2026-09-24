# css-reset-generator — competitor analysis (2026-09-24)

Scan run BEFORE implementing, so the descriptor shipped with the table-stakes already in it.
All notes are **paraphrased**; no competitor copy, branding, or trademarks were copied, and no
competitor stylesheet was reproduced verbatim — every rule this tool emits was authored here from
the documented *rationale*, not pasted.

## Competitors reviewed

| # | tool / source | reachable | what it is |
| - | ------------- | --------- | ---------- |
| 1 | FastTool "CSS reset generator" (fasttool.app) | **no — HTTP 410 on fetch**, replaced (profile below is search-index level only) | the closest direct competitor: a configurable generator |
| 2 | Josh W. Comeau — "A Modern CSS Reset" | yes | the reference modern reset most generators copy their default preset from |
| 3 | Piccalilli / Andy Bell — "A (more) Modern CSS Reset" | yes | the other canonical modern reset |
| 4 | Elly Loel — "Modern CSS Reset" | yes | an explicitly *opinion-toggle* reset (author marks rules optional) |
| 5 | Meyerweb — "CSS Reset v2.0" (public domain) | yes | the classic aggressive zero-out everyone still ships as a preset |

The unreachable competitor (1) was replaced by (5) per the scan rule — five real profiles, not four.

### 1. FastTool CSS reset generator — *unreachable (410), index-level profile*
- Presets advertised: a classic Meyer-style reset, a normalize-style sheet, a "modern" reset, and a
  Tailwind-Preflight-style sheet.
- Per-section toggles (box-sizing, margins, headings, links, lists).
- Output: formatted **or minified**, copy + download, runs client-side.
- Because the page itself would not load for the scan, its feature list is treated as *claimed*, not
  verified. Every capability listed is independently table-stakes across (2)–(5) anyway.

### 2. Josh W. Comeau — A Modern CSS Reset
- Rules: `box-sizing: border-box` on `*` + pseudo-elements; margin zeroing (with `dialog` spared);
  `body { line-height: 1.5 }`; `-webkit-font-smoothing: antialiased`; media elements
  (`img/picture/video/canvas/svg`) set to `display: block; max-width: 100%`; form controls
  `font: inherit`; `overflow-wrap: break-word` on paragraphs/headings; `text-wrap: pretty` on
  paragraphs and `balance` on headings; `isolation: isolate` on framework roots (`#root`, `#__next`);
  a `prefers-reduced-motion: no-preference` guard around a keyword-interpolation opt-in.
- Rationale-per-rule is the copy pattern: each rule is explained, not just listed.

### 3. Piccalilli / Andy Bell — A (more) Modern CSS Reset
- Adds over (2): `text-size-adjust: none` (with `-moz-`/`-webkit-` prefixes) on `html`;
  `margin-block-end` logical-property zeroing; `list-style: none` only on `ul[role="list"]`/
  `ol[role="list"]` (Safari/VoiceOver semantics bug); `body { min-height: 100vh }`;
  `line-height: 1.1` on headings and interactive elements; `a:not([class])` colour/skip-ink;
  `textarea:not([rows]) { min-height: 10em }`; `:target { scroll-margin-block: 5ex }`.
- Notably does **not** ship a reduced-motion block or `scroll-behavior`.

### 4. Elly Loel — Modern CSS Reset
- The "toggleable opinions" model our backlog row asks for: the author explicitly marks rules as
  optional (smooth scrolling behind a motion-preference query, `textarea { resize: vertical }`,
  focus animations).
- Also: `:where()`-based low-specificity selectors so the reset is trivially overridable; dynamic
  viewport height on `body`; `cursor: pointer` on interactive elements and `not-allowed` on
  disabled ones; form controls inheriting colour and letter/word spacing; `role="list"` handling.

### 5. Meyerweb — CSS Reset v2.0 (public domain)
- The aggressive baseline: a long element list zeroed to
  `margin/padding/border: 0; font-size: 100%; font: inherit; vertical-align: baseline`, plus
  `list-style: none` on `ol/ul`, `quotes: none` on `blockquote/q` with empty `::before/::after`
  content, and `border-collapse: collapse; border-spacing: 0` on tables.
- License is public domain; even so, the `zero-out` section here was re-authored (own element list,
  own ordering, own formatter) rather than pasted.

## Table-stakes → where each landed

| table-stake (seen at ≥1 competitor) | verdict | how |
| ----------------------------------- | ------- | --- |
| Preset flavours (modern / minimal / normalize-style / classic zero-out / preflight-style) | **in-model** | `preset` enum + one `[[example]]` preset chip each |
| Per-section opt-in / opt-out toggles | **in-model** | `include` + `exclude` tag-list pills over a 29-id section vocabulary |
| `box-sizing: border-box` on `*` and pseudo-elements | **in-model** | section `box-sizing` |
| Margin zeroing on flow content | **in-model** | section `margin` |
| Padding zeroing on lists/grouping elements | **in-model** | section `padding` |
| `body { line-height: … ; min-height: … }` | **in-model** | section `body-defaults` + `line_height` slider + `body_min_height` enum (`100svh`/`100dvh`/`100vh`/none — the units the three modern resets disagree about) |
| Font smoothing | **in-model** | section `font-smoothing` |
| `text-size-adjust` (mobile font inflation) | **in-model** | section `text-size-adjust` |
| Block-level, max-width media | **in-model** | section `media` |
| Form controls inherit typography | **in-model** | section `forms` |
| Sensible `textarea` default | **in-model** | section `textarea` |
| `role="list"` list-marker removal | **in-model** | section `lists` |
| Unconditional list unstyling (preflight/classic style) | **in-model** | section `lists-unstyled` |
| Unclassed link defaults | **in-model** | section `links` |
| Heading `line-height` + `text-wrap: balance` | **in-model** | section `headings` |
| Headings inherit size/weight (preflight style) | **in-model** | section `headings-unstyled` |
| `text-wrap: pretty` on paragraphs | **in-model** | section `text-wrap` |
| `overflow-wrap: break-word` | **in-model** | section `overflow-wrap` |
| `prefers-reduced-motion` guard | **in-model** | section `reduced-motion` |
| Optional smooth scrolling behind a motion query | **in-model** | section `smooth-scroll` |
| `:target { scroll-margin-block }` | **in-model** | section `scroll-margin` |
| Root stacking-context isolation | **in-model** | section `isolation` |
| Pointer / not-allowed cursors | **in-model** | section `interactive` |
| Visible `:focus-visible` ring | **in-model** | section `focus-visible` |
| Table border collapsing | **in-model** | section `tables` |
| Monospace stack + `font-size: 1em` (normalize/preflight) | **in-model** | section `monospace` |
| `abbr[title]` dotted underline (normalize) | **in-model** | section `abbr` |
| `sub`/`sup` line-box fix (normalize) | **in-model** | section `sub-sup` |
| Predictable `hr` (normalize) | **in-model** | section `hr` |
| Button chrome stripping (preflight) | **in-model** | section `button-reset` |
| Classic aggressive zero-out | **in-model** | section `zero-out` (emitted first so later opinions win) |
| Low-specificity `:where()` selectors | **in-model** | `selector_style` enum |
| Minified output | **in-model** | `minify` checkbox |
| Section comments on/off | **in-model** | `comments` checkbox |
| Copy button / download | **in-model, already platform** | the page generator gives every `format = "text"` tool Copy + Download + Reset |

### Beyond every competitor (our differentiators)
- `@layer` wrapping (`layer` param) — none of the five emits a cascade-layer wrapper, which is the
  modern way to keep a reset permanently overridable.
- Configurable `indent` (0–8) for the formatted output.
- The same generator is reachable from chat and the CLI, not just a web form.

## Out-of-model (considered, NOT built)
- **Live preview iframe** rendering unstyled HTML before/after the reset — needs a sandboxed
  document and a curated demo page; the page model here is one deterministic text output.
- **Saving/sharing named configurations to an account** — no accounts, no server.
- **Direct "download as `reset.css` into a repo/PR"** integrations (GitHub/Gist push) — needs auth
  and a backend.
- **Auto-detecting which reset a pasted site already uses** — needs to fetch and parse third-party
  pages; the browser-local model can't fetch cross-origin CSS.
- **Bundling the exact byte-for-byte upstream normalize.css / Preflight releases** — that would be
  redistributing someone else's stylesheet rather than generating one; the `normalize` and
  `preflight` presets are *style-alike*, authored here, and the page says so.

## UX control patterns matched
- Preset buttons → `[[example]]` preset chips (one per preset, one click to prefill + run).
- Per-section toggles → tag-list pill fields (add/remove) instead of a comma-separated text box.
- Numeric opinion (line height) → `kind = "slider"`; indent → `kind = "slider"`.
- Formatted/minified switch → a real checkbox, deep-linkable via `?minify=true`.

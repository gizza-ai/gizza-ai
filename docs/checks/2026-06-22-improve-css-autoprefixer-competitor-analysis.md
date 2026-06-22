# css-autoprefixer — competitor analysis (2026-06-22)

New tool. Surfaces verified: chat skill (drift-guard schema test), CLI (`gizza tool
css-autoprefixer`), standalone page (Playwright `tool-page-css-autoprefixer.spec.ts`, 3
specs incl. query-param deep-link). Pure-Rust, hand-rolled declaration scanner — no
external CSS-parser dep; runs identically on every backend.

## Competitors surveyed

1. **Autoprefixer playground** (autoprefixer.github.io) — the canonical PostCSS
   Autoprefixer engine in the browser. Full Browserslist control; the gold standard for
   correctness/coverage.
2. **goonlinetools.com/autoprefixer** — client-side, "CSS never leaves your browser".
   Paste-in/paste-out, no target picker.
3. **FWD Tools Autoprefixer** — wraps the real PostCSS Autoprefixer with a Browserslist
   target selector.
4. **CSS Drive CSS AutoPrefixer** — two-column unprefixed→prefixed, plus an option list.
5. **CSS-Tricks "Autoprefixer" reference** — documents the canonical behavior (prefix by
   Can I Use data, Browserslist-driven, removes outdated prefixes).

## Capability diff (us vs. them)

| Capability | gizza | autoprefixer.io / FWD | goonlinetools / CSS Drive |
|---|---|---|---|
| Paste CSS → prefixed CSS | yes | yes | yes |
| Property prefixes (user-select, appearance, backdrop-filter, clip-path, mask, hyphens, text-size-adjust, …) | yes | yes | yes |
| Property renames (tab-size→-moz-/-o-) | yes | yes | partial |
| Value prefixes (display:flex→-webkit-box/flex/-ms-flexbox, position:sticky, intrinsic sizes) | yes | yes | partial |
| Idempotent (skip already-present prefix) | yes (default; `dedup`) | yes | varies |
| Comment / string / custom-property safe | yes | yes | varies |
| 100% client-side / private | yes | partial | yes (goonlinetools) |
| Programmatic API (chat skill + CLI) | yes | no | no |
| Deep-link via query params | yes | no | no |

## Gaps closed in this build

- **Cascade order** — prefixed clones emitted *before* the standard declaration so a
  fully-supporting browser uses the unprefixed form (matches Autoprefixer).
- **Idempotency** — `dedup=true` default so re-running on already-prefixed CSS does not
  duplicate prefixes (matches the "removes/avoids redundant prefixes" expectation).
- **Safety** — declarations inside `/* … */` comments, quoted strings, and CSS custom
  properties (`--var`) are left byte-for-byte untouched; non-prefixed properties pass
  through unchanged (only the touched declaration's spacing is normalized to `prop: value`).
- **Coverage of the modern prefix set** — curated table of the property-, rename-, and
  value-prefixes still required by current targets (flexbox, sticky, masks, hyphens,
  intrinsic sizes, etc.).

## Out-of-model gaps (NOT built — documented, not copied)

- **Browserslist / target-version selection.** The canonical engines prefix *by Can I Use
  data for a chosen target list*. We ship a single curated "current targets" set rather
  than a configurable Browserslist, because bundling the full Can I Use dataset + a
  Browserslist resolver is out of scope for a pure-compute block. This is the main
  coverage delta vs. autoprefixer.github.io / FWD Tools.
- **Prefix *removal*** of outdated prefixes from already-prefixed input (we add, and
  de-dup, but do not strip historical prefixes the modern target no longer needs).
- **Grid `-ms-` autoplacement translation** (the complex IE grid track/area rewriting).
  We emit `display:-ms-grid` but do not translate `grid-template-*`/`grid-gap` into the
  IE syntax.

No competitor copy, branding, or trademarks were reproduced. The prefix tables are
derived from public Can I Use / MDN prefix knowledge, hand-authored.

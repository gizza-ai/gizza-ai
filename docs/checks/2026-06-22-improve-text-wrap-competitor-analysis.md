# text-wrap — competitor analysis (2026-06-22)

Tool: `blocks/text-wrap` — hard-wrap (reflow) text to a fixed column width.
Surfaces verified: chat block (`wafer build` validate/instantiate OK), CLI
(`gizza tool text-wrap`), standalone page (`/tools/text-wrap/`, 3 Playwright
tests pass: wrap-to-width, break-long-words toggle off, query-param deep-link).

## Top competitors surveyed

1. **onlinetexttools.com — Wrap Words in Text** — set max line width; greedy word
   wrap; option to break long words; preserves existing newlines.
2. **freetexttools.org — Wrap Text to Column Width** — width 20–120; "smart word
   breaking"; preserve paragraphs; pitched for email/code/docs.
3. **browserling.com — Word Wrap** — minimal: paste, set width, wrap; no options
   beyond width.
4. **easecloud.io — Text Wrap** — width presets (80/100/120); preserves word
   boundaries.
5. **appzaza.com — Word Wrapper** — inserts breaks at the nearest space before the
   column, does not break words.

## Capability diff (competitor → us)

| Capability | Competitors | text-wrap |
| --- | --- | --- |
| Set column width | all | yes (`width`, default 80, 1–10000) |
| Greedy word wrap (don't break words) | all | yes (default) |
| Break a word longer than the width | onlinetexttools (opt), others no | yes, toggle `break_long_words` (default on) |
| Preserve existing line breaks / paragraphs | most | yes (each source line reflowed independently; blank lines kept) |
| Collapse runs of inner whitespace | varies | yes |
| **Preserve leading indentation** on continuation lines | rarely offered | **yes** (`preserve_indent`, default on) — differentiator for indented blocks / list items |
| Unicode-aware width (counts chars, not bytes) | varies | yes (counts Unicode scalar values) |
| Private / in-browser / no sign-up | most claim it | yes (pure-Rust wasm, runs locally) |

## Gaps closed / parity

- All core competitor features (width, greedy wrap, optional long-word breaking,
  paragraph/newline preservation, in-browser privacy) are present.
- Added an indent-preservation option most competitors lack, which keeps indented
  paragraphs and bullet/numbered list items aligned after wrapping.
- Width bounds and option semantics are single-sourced from the descriptor →
  chat schema (drift-guard test) and shared by core/CLI/page.

## Out-of-model / not built (intentional)

- **Display-width (CJK/east-asian width) wrapping** — would need a unicode-width
  table; current behavior counts scalar values and is documented as such in the
  page FAQ. Left as a future enhancement, not a gap vs. the surveyed competitors
  (which also count characters).
- **Hanging-indent / first-line-indent reformatting** — out of scope; this tool
  preserves the source indent rather than re-indenting.
- No competitor copy, branding, or trademarks were used.

## Sources

- https://onlinetexttools.com/wrap-words-in-text
- https://www.freetexttools.org/word-wrap-text/
- https://www.browserling.com/tools/word-wrap
- https://www.easecloud.io/tools/text/text-wrap/
- https://appzaza.com/word-wrapper

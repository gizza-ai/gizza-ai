# html-comment-stripper — competitor analysis (2026-08-16)

Scan done BEFORE implementing. All observations are paraphrased from public product
pages/READMEs; no competitor copy, branding or trademark text is reproduced here or in the
shipped tool.

## Field

| # | Competitor (kind) | What it does | Options exposed | Reported stats |
|---|---|---|---|---|
| 1 | hextostring — "remove html comments" (single-purpose page tool) | Paste HTML, click a button, get commentless HTML | 2 checkboxes: keep conditional comments (**on** by default), remove empty lines (**off**) | none |
| 2 | webtexttools — remove HTML comments (page tool) | Paste **or upload** a file, strip comments | 3 independent checkboxes: HTML `<!-- -->`, CSS `/* */`, JS `//` + `/* */` | none |
| 3 | imageonline — remove HTML comments (page tool) | Two-pane paste → cleaned output | **none** (single fixed behaviour) | input chars, comments found, output chars, % size reduction |
| 4 | onlineminitools — remove comments (multi-language page tool) | Pick a language from a ~17-entry list, strip its comments | language selector only | none |
| 5 | html-minifier-terser (npm library, the de-facto reference implementation) | Full minifier; comment removal is one flag | `removeComments` (default **false**), `ignoreCustomComments` (default keep `/^!/` bang and `/^\s*#/` hash comments), `processConditionalComments` (default false) | none |

Adjacent reference read for defaults only: htmlnano's `removeComments` issue thread, where the
requested shape is a tri-state — keep all / remove all / remove all *except* conditional and
marker comments.

## Table stakes (what every serious tool in this field has)

1. **Paste-in / result-out with copy + download.** The gizza page generator gives this for free
   (`format = "text"` renders a Download link and the copy control).
2. **Keep conditional comments by default.** 3 of the 5 either default to keeping IE conditional
   comments or treat them as a separate switch. This is the single most-requested behaviour in the
   field, and the one a naive `<!--.*?-->` regex gets wrong.
3. **Preserve "important"/banner comments.** html-minifier-terser's default ignore list (`^!` bang,
   `^\s*#` hash) is the industry convention for licence banners and SSI directives; tools that
   ignore it silently delete legal notices.
4. **Optional blank-line tidy-up** after removal (competitor 1's second checkbox) — a comment on
   its own line otherwise leaves an empty line behind.
5. **Don't touch CSS/JS syntax by accident** (competitor 1 states this explicitly as a safety
   property; competitor 2 makes it opt-in the other way).
6. **Some visible confirmation of what happened** — competitor 3's counts + % reduction is the only
   feedback surface in the field, and it is the most-praised part of that page.

## Gap list — in-model vs out-of-model

### In model, shipping in this build

| Gap | Source | Decision |
|---|---|---|
| Keep IE conditional comments (`<!--[if …]> … <![endif]-->`), default on | 1, 5 | `keep_conditional`, `Param::boolean` default **true** |
| Keep SSI / hash directives (`<!--#include … -->`) | 5 (`/^\s*#/`), Server Side Includes spec | `keep_ssi`, default **true** |
| Keep bang/banner comments (`<!--! … -->`) | 5 (`/^!/`) | `keep_bang`, default **true** |
| Arbitrary keep-list by pattern (`ignoreCustomComments`) | 5, htmlnano thread | `pattern` + `pattern_mode = keep`, a regex over the comment's inner text |
| Remove *only* matching comments (the CMS-marker case: `<!-- wp:… -->`, analytics markers) | htmlnano thread ("remove all except…" inverted) | `pattern_mode = only` — nobody in the field ships this |
| Strip CSS `/* … */` comments inside `<style>` | 2 | `remove_css_comments`, default **false**, string-aware |
| Blank-line tidy-up | 1 | `blank_lines = keep \| trim \| collapse` (an enum, not a bool — `trim` drops lines that became blank, `collapse` also folds runs of blank lines) |
| Counts / bytes saved / % reduction | 3 | `output = report` |
| Never treat `<!--` inside `<script>`/`<style>`/`<textarea>`/`<title>` or inside a quoted attribute value as a comment | 1 (claimed), everyone else regex-based | Raw-text-aware, quote-aware scanner in core — the main correctness delta |
| Worked example + preset chips | — | 5 `[[example]]` chips on the page |

### Out of model / deliberately not built

| Feature | Why not |
|---|---|
| JavaScript comment stripping inside `<script>` (competitor 2) | Correct JS comment removal needs a real lexer: `//` inside a string or a regex literal, and the division-vs-regex ambiguity, make the naive version silently corrupt code. gizza already ships `js-minify` and `js-css-minifier` for that job; the FAQ points there rather than shipping a lossy version. |
| Multi-language comment stripping (competitor 4) | A different tool. `code-comment-extractor` covers the source-code side of the repo. |
| File upload | The page's field surface is paste-only for text tools; the CLI takes the markup as an argument and the page Download link covers the round trip. |
| "Nested comments handled safely" (competitor 3's claim) | HTML comments **do not nest** — per the HTML parser, `<!-- a <!-- b --> c -->` ends at the first `-->`. Matching spec behaviour is correct; the FAQ says so explicitly instead of claiming a feature that does not exist. |
| "Files of any size" (competitor 3's claim) | Untrue of any in-browser tool. This one states a 5,000,000-byte cap and enforces it at the boundary (tested). |
| Minification / whitespace collapsing | `html-minifier` already ships that. This tool's contract is the opposite: output is byte-identical to the input except for the comments it removed. |

## Positioning against the repo's own blocks

- `html-minifier` has a `remove_comments` flag, but it **always** collapses whitespace, normalizes
  tag internals and rewrites the document — there is no comments-only mode, and it has no
  conditional/SSI/bang handling and no report.
- `html-sanitizer` drops comments by default but also strips tags and attributes against an
  allowlist.
- Neither offers a byte-preserving strip, a keep/only pattern, or an audit of what was removed.
  That is this tool's slot.

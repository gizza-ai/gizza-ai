# format-css — competitor analysis (2026-07-30)

Scan of the top public CSS formatters/beautifiers before implementing. Findings paraphrased,
never copied. Sources reviewed: CSS Portal CSS Formatter, CodeBeautify SCSS Beautifier (403 on
fetch, from search summary), Toolraxy CSS Formatter, Flipper File CSS Formatter, DeCodeIt CSS
Beautifier, Ubercompute SCSS/LESS Formatter.

## Table stakes observed

| Capability | Competitors | Our decision |
|---|---|---|
| Indent width 2 / 4 / tab | all | **in** — `indent` (0–8) + `indent_char` (space/tab), default 2 spaces |
| One declaration per line, space after `:`, trailing `;` | all | **in** — always normalized |
| Property sorting: Off / A–Z / Idiomatic-grouped | CSS Portal, Toolraxy, Flipper File | **in** — `sort` = none / alphabetical / grouped |
| Multi-selector split (comma selectors on own lines) | CSS Portal, Toolraxy | **in** — `selectors_per_line` (default on) |
| Uppercase / normalize hex colors | Toolraxy, CSS Portal (as "Optimise") | **in** — `uppercase_hex` (default off) |
| SCSS / LESS nesting support | DeCodeIt, Ubercompute, CodeBeautify (many plain-CSS tools do NOT) | **in** — brace-recursive parser handles nested rules, `&`, `@media`/`@mixin`/`@include`, `$`/`@` vars and `//` line comments |
| Preserve comments (`/* */` and `//`) | most | **in** — comments preserved and re-indented |

## Idiomatic ("grouped") order

CSS Portal / Toolraxy group by concern: custom properties → positioning → box model → border →
background/color → typography → visual effects → interaction. We implement a curated concentric
order for the common properties and fall back to alphabetical for anything not in the table, so
the ordering is deterministic and testable.

## Considered, out of model (not built — noted for honesty)

- **Minify** — already covered by the shipped `js-css-minifier` block; out of scope here.
- **Value-level optimization** (drop zero units, hex shorthand `#ffffff`→`#fff`, named-color
  swaps, shorthand collapsing for margin/padding) — that is an optimizer, a distinct capability;
  we only normalize whitespace/casing/order, never rewrite values (safe, lossless).
- **Multi-line expansion of gradients / grid-template values** — a presentation preference that
  risks changing value semantics; declined to keep output lossless.

## Differentiation vs existing `code-formatter`

`code-formatter` beautifies plain CSS (among HTML/JS/JSON) with **whitespace only** — it never
reorders or normalizes. `format-css` is CSS/SCSS/LESS-specific and adds declaration ordering
(alphabetical/grouped), hex normalization, and per-selector line splitting — capabilities
`code-formatter` explicitly does not provide.

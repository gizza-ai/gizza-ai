# js-beautify — competitor analysis (2026-06-23)

`/create-next-tool` backlog pick. Pure-Rust, dependency-free token-aware
pretty-printer that re-indents minified/obfuscated JavaScript into readable
source. Pure → runs on ALL backends including the chat Service Worker.
Surfaces verified: **chat (block.wasm) + CLI + standalone page** (text output).
Research via `WebSearch` + `WebFetch` of beautifier.io, paraphrased.

Not a duplicate: existing blocks cover JSON (`json-beautify`), HTML
(`html-formatter`), XML (`xml-formatter`), SQL (`sql-formatter`) — none format
JavaScript. `javascript-runner` is skiplisted (needs a JS *engine*); this tool
only re-formats whitespace, no engine.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| beautifier.io (js-beautify lib) | indent size + tabs; brace style; preserve-newlines cap; line-wrap width; comma-first; break chained methods; detect packers; unescape strings; JSX/e4x | capabilities |
| XKit (Prettier) | ES6+/JSX/Flow/TS; 100% in-browser/local | capabilities / privacy |
| FlexiTools | minified→readable, ES6, custom indent | capabilities |
| DotIAM Tools | 2/4-space or tabs; file upload + download; frameworks | UX / capabilities |
| codebeautify / prepostseo | unminify; output stats (lines, size before/after) | UX |

## Gap diff vs our tool
Our tool: re-indents to one statement / object-member / array-element per line,
indents nested `{ ( [`, with `indent` 1–8 **spaces or a tab** (`indent_char`).
Strings, template literals, regex and comments are preserved verbatim, ternary
`?:` is spaced, statements after `}` break to a new line, `else`/`catch`/`while`
stay on the brace line, empty `{}` stays inline. Covers the core
"minified → readable + choose indentation (spaces/tabs)" capability that every
competitor leads with.

**Closed this iteration:**
- **Tab indentation** (`indent_char` = space|tab) — every competitor offers it;
  added as an enum param across chat/CLI/page (page renders a `<select>`).

**In-model gaps considered, deferred (fit the form/chat model; good follow-ups):**
- **Brace style** (`collapse` vs braces-on-own-line / Allman). Cheap to add as
  another enum once a style is chosen; deferred to keep the schema bounded.
- **Preserve-newlines cap** — keep up to N author blank lines. Our tokenizer
  currently collapses all blank lines; tracking newline runs is a sized add.
- **Break chained methods** (`.then().then()` → one call per line). Doable with
  the existing token stream; deferred.
- **Unescape printable `\xNN`/`\uNNNN` in strings** — a string-content transform;
  deferred (and it changes string bytes, against our "verbatim literals" default).
- **Line-wrap at N columns** — needs a width model; deferred.
- **Output stats** (lines / size before-after) — a page/CLI presentation add, not
  a core capability.

**Out-of-model:** rich editor UI with dropdown menus + live preview + dark mode
(a custom app, not a form/chat tool); file upload/download UX (the page takes a
text field); running the JS (engine — see the `javascript-runner` skiplist).
JSX/TSX-specific reformatting is partial: the tokenizer treats JSX as words/punct
so it won't break, but it isn't JSX-aware — documented, not claimed.

## Tested
core unit (19: empty error, block/nested indent, indent width, tab indent,
string/regex/template verbatim, ternary vs object colon, for-loop inline `;`,
empty-block inline, line-comment break, statement-after-`}` break, else-on-brace,
member-access + division tightness) + drift-guard schema test · `wafer build`
validates block.wasm (pure-Rust → also runs in the chat SW) · CLI on minified
input incl. `indent=4` and `indent_char=tab` + empty-input error · Playwright
page (4: 2-space, custom width, ternary spacing, tab select). All green.

> Original work only — no competitor copy, branding, or trademarks copied. The
> tool name "js-beautify" is the backlog slug / generic category term.

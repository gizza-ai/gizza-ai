# syntax-highlighter — competitor analysis (2026-06-22)

## Tool

`gizza tool syntax-highlighter` — highlight a code snippet for a given language and
return styled output as either self-contained inline-styled **HTML** (a `<pre>` with
colored `<span>`s and a theme background, no external stylesheet) or **ANSI** 24-bit
terminal escape codes. Pure-Rust (syntect `default-fancy` engine + bundled Sublime
syntaxes/themes), so it runs on every backend including the chat Service Worker.
Inputs: `code` (required), `language` (hint, e.g. `rust`/`python`/`bash`; unknown →
plain text), `theme` (syntect theme name, default `base16-ocean.dark`; unknown →
default), `format` (`html` default | `ansi`). Output: the highlighted markup/escape
string.

Surfaces: **chat + CLI + page** (text output renders on the page). Verified:
`cargo test --workspace` (12 tests: 11 core + 1 chat-schema drift guard), `wafer
build` (block validates + instantiates, 2276.4 KiB — syntect's `html` module +
fancy-regex link clean in wafer), CLI emits inline-styled HTML with `&lt;` escaping /
ANSI true-color escapes / errors on empty input (exit 1), and Playwright drives the
page for both HTML and ANSI formats.

## Competitors surveyed

- **highlight.js** (highlightjs.org) — the dominant JS library; auto-language detection,
  ~190 languages, ~240 CSS themes; outputs HTML with CSS *class names* (needs an
  external/embedded stylesheet to color).
- **Prism.js** (prismjs.com) — lightweight JS highlighter; class-based HTML output, line
  numbers / line-highlight / copy plugins; many themes via CSS.
- **Shiki** (shiki.matsu.io) — TextMate-grammar highlighter using VS Code themes;
  outputs HTML with **inline styles** (no CSS needed) — the closest match to our HTML mode.
- **Pygments** (pygments.org) — Python library/CLI; `pygmentize`; HTML (inline or class),
  ANSI/256-color terminal, RTF, LaTeX, SVG, image (PNG) formatters; ~500 lexers, ~40 styles.
- **CodeMirror / Monaco** — editor components with highlighting (interactive, not a
  one-shot snippet→markup converter).
- **`bat`** (github.com/sharkdp/bat) — syntect-based `cat` replacement; ANSI 24-bit
  terminal output, themes; the closest reference for our ANSI mode.
- **GNU `source-highlight`**, **Rouge** (Ruby), **carbon.now.sh / ray.so** (these last two
  produce a PNG *image*, like our sibling `code-screenshot` block).

## Capability diff & gap ranking (fit-to-model)

| Capability | Competitors | gizza syntax-highlighter | Status |
|---|---|---|---|
| Highlight code → HTML | highlight.js, Prism, Shiki, Pygments | **yes** — `<pre>` + `<span>`s | ✅ at parity |
| **Self-contained** inline-styled HTML (no external CSS) | Shiki, Pygments (`noclasses`) | **yes** — every span carries an inline `style`, plus a theme background on `<pre>` | ✅ at parity (ahead of class-only highlight.js/Prism) |
| ANSI / terminal output | Pygments, `bat`, source-highlight | **yes** — 24-bit true-color escapes ending in a reset | ✅ at parity |
| HTML entity escaping (`<`/`>`/`&`) | all | **yes** — verified `a < b` → `&lt;` so markup is safe to embed | ✅ at parity |
| Many languages | 190+ (highlight.js), ~500 (Pygments) | **100+** via syntect's bundled Sublime grammars, with friendly aliases (`rust`/`rs`, `py`, `js`, `cpp`, `cs`, `go`, `yaml`, `md`…) | ✅ at parity for common languages |
| Multiple themes | hundreds (CSS) | syntect defaults: `base16-ocean.{dark,light}`, `Solarized (dark/light)`, `base16-eighties.dark`, `base16-mocha.dark`, `InspiredGitHub` (light), `base16-ocean.dark` default | ✅ covers the common dark/light/Solarized set |
| Unknown language → graceful fallback | highlight.js (auto), Pygments (`TextLexer`) | **yes** — falls back to uncolored plain text, never errors | ✅ at parity |
| 100% local, no upload | client-side JS libs do; pastebins/online tools don't | **yes** — pure WASM in-browser / CLI; code never leaves the device | ✅ ahead of server-side online highlighters |
| Automatic language detection | highlight.js, Pygments (guess_lexer) | manual `language` hint (omitted → plain text) | ⏭️ out of current model |
| Line numbers / line-highlight / copy button | Prism plugins, Shiki transformers | not built (this tool emits markup; rendering chrome is the consumer's job) | ⏭️ out of model |
| LaTeX / RTF / SVG / PNG-image formatters | Pygments; carbon/ray.so (PNG) | not built — PNG image of code is the sibling `code-screenshot` block | ⏭️ separate tool / out of model |

### In-model gaps closed
All in-model capabilities a one-shot snippet→styled-output highlighter should have are
present: HTML and ANSI output, **self-contained inline styling** (matching Shiki /
Pygments `noclasses` rather than the class-only output of highlight.js/Prism that
needs a separate stylesheet), safe HTML escaping, 100+ languages with friendly
aliases, a set of dark/light/Solarized themes, and a graceful plain-text fallback for
unknown languages. The chat skill description, manifest, and page copy spell out the
HTML-vs-ANSI behaviour, the language hint, and the theme default so the LLM and users
set expectations correctly. No competitor copy, branding, or trademarks were used.

### Out-of-model / deliberately not built (with reasons)
- **Automatic language detection.** highlight.js and Pygments guess the language from
  the source. Robust guessing needs a trained classifier / heuristic engine; syntect
  has no detection API. Out of the pure-deterministic model — we take an explicit
  `language` hint and fall back to plain text, which is predictable and never wrong-guesses.
- **Rendering chrome (line numbers, line-highlight ranges, copy button).** This tool
  returns the highlighted *markup*; line numbers and interactive affordances are the
  job of whatever renders that markup (a page, a blog template). Adding them would
  bake one consumer's layout choices into the output string.
- **PNG / image output.** A shareable *picture* of code (carbon/ray.so style) is the
  existing `code-screenshot` block — a deliberately separate tool with image-bytes
  output. This tool is the text-markup counterpart (distinct surface: it has a page;
  `code-screenshot` has none).
- **LaTeX / RTF / SVG formatters.** Pygments ships these; they're niche output targets
  that don't fit the page's text-render model and weren't part of the tool's scope.

## Notes / limitations
- Highlighting is grammar-based (Sublime/TextMate syntaxes via syntect), not a full
  compiler, so edge-case tokens in some languages may color imperfectly — the same
  trade-off `bat`, Shiki, and VS Code make.
- Theme set is syntect's bundled defaults (8 themes); this is a smaller palette than
  highlight.js's hundreds of CSS themes, but covers the common dark/light/Solarized
  needs and keeps the tool a single self-contained WASM blob.

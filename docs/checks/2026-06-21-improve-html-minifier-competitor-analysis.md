# html-minifier — competitor analysis & differentiation

**Tool:** `gizza-ai/html-minifier` — minify HTML by collapsing whitespace,
removing comments, and trimming redundant whitespace.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `html-minifier-terser` (kangax) | Library/CLI | The reference, very configurable, but needs Node + a config. |
| Online "HTML minifier" sites | Web | Common, but most **upload your markup**, are ad-heavy, and quality/safety varies. |
| Build-tool plugins (webpack/vite) | App | Great in a pipeline, useless for a quick one-off minify. |
| Manual find/replace | DIY | Error-prone; easy to break `<pre>` or inline spacing. |

## How gizza's tool is better / different

1. **Local — markup never uploaded.** Runs in WASM (chat SW + CLI + page).
2. **Safe by default.** Significant **inline spacing is preserved** (a real space
   between `<b>a</b> <b>b</b>` survives), and **`<pre>`/`<textarea>`/`<script>`/
   `<style>` are kept verbatim** — the two things careless minifiers break.
3. **Comment control.** Comments removed by default; flip it off to keep them
   (e.g. for licence banners or conditional comments).
4. **Tag whitespace normalized** without ever touching attribute *values* (a
   quote-aware scanner).
5. **Paired with `html-formatter`.** Minify ↔ pretty-print, same forgiving
   HTML-aware core (unlike `xml-formatter`, which needs well-formed XML).

## Verification

Seven core unit tests pin behavior: indentation collapse, comment removal (and
keeping when disabled), tag-whitespace normalization without altering values,
`<pre>` verbatim preservation, and inline-space preservation. **End-to-end CLI**
minified an indented, commented snippet to `<div><p>Hello <b>world</b></p></div>`
(comment gone, inline space kept). Page Playwright covers the default
(comments-stripped) and the unchecked (comments-kept) paths.

## Scope / honest limitations

- A whitespace/comment minifier — it does not minify inline CSS/JS, rewrite
  attribute quotes, or drop optional tags (kangax's deeper passes). It is safe
  and predictable rather than maximally aggressive.
- "Trimming redundant attributes" here means **whitespace** normalization inside
  tags, not removing `type="text/javascript"`-style redundancy.

## Possible future enhancements

- Optional collapse of boolean attributes (`disabled=""` → `disabled`).
- Optional inline `<style>`/`<script>` minification.
- "Conservative" mode that keeps all whitespace-only nodes as single spaces.

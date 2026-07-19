## About this tool

**JavaScript & CSS Minifier** shrinks either JavaScript *or* CSS by removing
everything that doesn't change how it behaves — comments, whitespace and line
breaks — and shows you exactly how many bytes you saved.

- **One tool, both languages.** Leave **Language** on *Auto-detect* and it works
  out whether you pasted JavaScript or CSS from the source, or force **CSS** /
  **JavaScript** if a snippet is ambiguous.
- **Safe by design.** The JavaScript path is token-aware (not a name-mangler):
  strings, template literals and regular expressions are kept verbatim,
  identifiers are never renamed, and ASI-significant line breaks are preserved.
  The CSS path is structural-only — it collapses whitespace and drops redundant
  separators but never rewrites a value, and it protects strings, `url(...)` and
  `calc(...)`/parenthesized expressions. So the output is guaranteed equivalent
  to the input.
- **Keeps license banners.** With **Remove comments** on, a `/*! … */` block or
  any comment containing `@license` / `@preserve` still survives.
- **Size report.** **Prepend size report** adds a one-line comment showing the
  original vs minified bytes and the percentage saved.

Everything runs **locally in your browser** via WebAssembly — your code is never
uploaded.

### Worked example

Paste this CSS with **Language** = *CSS*, **Remove comments** on and **Prepend
size report** on:

```css
.card {
  margin: 0 auto;   /* center it */
  color: #ff0000;
}

.card > .title {
  font-weight: bold;
}
```

You get:

```css
/* CSS: 98 B -> 63 B — 35.7% smaller (saved 35 B) */
.card{margin:0 auto;color:#ff0000}.card>.title{font-weight:bold}
```

Whitespace collapses, the trailing `;` before each `}` is dropped, the `>`
combinator loses its spaces, and the ordinary `/* center it */` comment is
stripped — but `#ff0000` is left exactly as written.

## FAQ

<details>
<summary>How does auto-detect decide between JavaScript and CSS?</summary>

It scores the source on language markers — JavaScript signals like `function`,
`=>`, `const`, `===`, `console.` versus CSS signals like `@media`, `:root`,
`!important`, units (`px`, `rem`) and `selector { prop: value; }` blocks. If the
CSS signals clearly win it minifies as CSS, otherwise as JavaScript. Real-world
JS and CSS separate cleanly; if a tiny or unusual snippet ever guesses wrong,
just set **Language** to **CSS** or **JavaScript** explicitly.

</details>

<details>
<summary>Does it rename variables or rewrite CSS values like a heavy minifier?</summary>

No. This is a whitespace/comment minifier, not a name-mangler or value
optimizer. JavaScript identifiers are never renamed and no code is reordered or
dropped. CSS values are left byte-for-byte — it will **not** shorten `#ffffff`
to `#fff`, strip leading zeros, or merge rules. That keeps the output guaranteed
equivalent to the input, which matters for hand-checked and vendor code. The
result is a little larger than an aggressive optimizer would produce.

</details>

<details>
<summary>How do I keep a license or copyright banner?</summary>

Banners survive comment removal automatically: any `/*! … */` block comment, or
one containing `@license` or `@preserve`, is treated as important and kept even
when **Remove comments** is on. This works for both JavaScript and CSS, since
`/* … */` is valid in both.

</details>

<details>
<summary>Can minifying break CSS like calc() or media queries?</summary>

No. The CSS minifier protects the inside of parentheses, so
`calc(100% - 10px)` keeps its spaces (removing them would break it), and it only
strips spaces around `>`, `+` and `~` at the top level where they're
combinators — never inside a value. Strings and `url(...)` contents are emitted
verbatim, and `@media`/`@keyframes` blocks minify normally.

</details>

## Limits & notes

- **Paste-only, single input.** There's no file upload, URL fetch, or bundling
  of multiple files — paste one JavaScript or CSS source at a time.
- **Structural minification only.** No value-level rewrites (hex shortening,
  zero stripping, rule merging) and no JavaScript identifier renaming — the
  trade for guaranteed-equivalent output.
- **The size report is a comment.** When **Prepend size report** is on, the
  first line of the output is a `/* … */` comment; untick it for pure minified
  code with nothing prepended.
- **Empty input is rejected** with a clear message rather than returning an
  empty result.

## About this tool

**CSS Formatter** pretty-prints (beautifies) minified or messy **CSS, SCSS, and LESS**. It puts
one declaration per line, normalizes `prop: value` spacing, indents nested rules, and preserves
your comments — all in your browser. Nothing is uploaded.

Beyond plain re-indenting it can, optionally:

- **Sort declarations** within each rule — alphabetically (A–Z) or by a concentric *grouped* order
  (custom properties → positioning → box model → border → background/color → typography → visual
  effects → interaction).
- **Split comma selectors** onto their own lines (`h1, h2` → `h1,` / `h2`).
- **Uppercase hex colors** (`#abcdef` → `#ABCDEF`).

It understands SCSS/LESS **nesting** (`&:hover`, `@media { … }`, `@mixin`/`@include`), `$`/`@`
variables, and `//` line comments — while leaving `url(http://…)` and quoted strings untouched.

### Example

Input:

```css
.btn{color:#abcdef;margin:0;&:hover{color:red}}
```

Output (defaults — 2-space indent, source order):

```css
.btn {
  color: #abcdef;
  margin: 0;
  &:hover {
    color: red;
  }
}
```

### Limits

- **Whitespace, casing, and order only** — values are never rewritten. It will not drop zero units,
  shorten `#ffffff` to `#fff`, collapse shorthands, or minify (that's a different, lossy operation —
  use a CSS minifier for that).
- Sorting reorders declarations, which can change the cascade for duplicate properties in the same
  rule — leave sort on **Keep source order** if a rule relies on later-wins duplicates.
- It's a forgiving formatter, not a validator: badly broken CSS is reindented as best it can, not
  rejected.

## FAQ

<details>
<summary>Does it work with SCSS and LESS, not just plain CSS?</summary>

Yes. Nested rules, the `&` parent selector, block at-rules like `@media`/`@mixin`, `@include`
statements, `$foo`/`@foo` variables, and `//` line comments are all handled. Plain CSS is just the
case with no nesting.

</details>

<details>
<summary>What does "grouped" sorting order look like?</summary>

Grouped (a.k.a. concentric or idiomatic) order lists custom properties first, then positioning,
then the box model, borders, background/color, typography, visual effects, and interaction. Any
property not in the built-in table falls back to alphabetical after the known ones, so the result is
always deterministic.

</details>

<details>
<summary>Will it change my colors or values?</summary>

No — formatting is lossless. The only value-level change is optional: turning on **Uppercase hex
colors** rewrites `#abcdef` to `#ABCDEF`. It never shortens hex, drops units, collapses shorthands,
or otherwise rewrites values.

</details>

<details>
<summary>Is my CSS uploaded anywhere?</summary>

No. The formatter runs entirely in your browser via WebAssembly — the CSS you paste never leaves
your machine.

</details>

<details>
<summary>Can it minify CSS?</summary>

No, this tool only beautifies (expands) CSS. Minification is the opposite, lossy transform — reach
for a dedicated CSS minifier if you want to shrink output.

</details>

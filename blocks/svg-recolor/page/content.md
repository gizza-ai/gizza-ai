## About this tool

**SVG Recolor** rewrites the colors of an SVG icon or illustration without
touching its shapes, paths or layout.

There are two ways to use it:

- **Swap specific colors** — fill the *Color map* with `from=>to` pairs, one per
  line or comma-separated:

  ```
  #000000 => #ffffff
  red => #00ff00
  ```

  Each source color is matched no matter how it's written in the file — `#fff`,
  `#ffffff`, `#ffffffff` (with alpha) and `rgb(255, 255, 255)` all match a source
  of `#ffffff`, and named colors like `red` work too.

- **Recolor everything (monochrome)** — put a single color in the *Monochrome*
  field to tint the whole graphic one color, e.g. to turn a multi-color logo into
  a single-color glyph. This overrides the color map when set.

It rewrites colors wherever SVG puts them:

- presentation attributes — `fill`, `stroke`, `stop-color`, `color`,
  `flood-color`, `lighting-color`, `solid-color`;
- inline `style="fill:…;stroke:…"` declarations;
- rules inside `<style>` blocks.

It's intentionally **safe**:

- **Path data, geometry and structure are never changed** — only color values
  are rewritten.
- **`none`, `transparent` and `currentColor`** are preserved — they aren't real
  colors, so they're left as-is.
- Colors you don't map are left untouched.

Everything runs **locally in your browser** via WebAssembly — your SVG is never
uploaded.

## FAQ

<details>
<summary>Do I have to write the color exactly as it appears in the file?</summary>

No. Colors are compared by their canonical RGB(A) value, so a map entry of
`#ffffff => #000000` also rewrites occurrences written as `#fff`, `#FFFFFF`,
`#ffffffff` or `rgb(255, 255, 255)`. Named colors work too — the tool knows the
16 basic SVG/HTML names plus common extras like `orange`, `gold`, `indigo` and
`violet`; an unrecognized name is treated as a non-color and left alone.

</details>

<details>
<summary>What separators does the color map accept?</summary>

Each pair can use `=>`, `->`, `:` or `=` between the source and target color,
and pairs can be separated by newlines, commas or semicolons — so
`#000=>#fff, red -> #00ff00` and one-pair-per-line both parse. An entry with a
missing separator or an unparseable color on either side is reported as an
error instead of being silently skipped.

</details>

<details>
<summary>Why didn't monochrome mode recolor every part of my icon?</summary>

Monochrome replaces every *real* color, but `none`, `transparent` and
`currentColor` are keywords, not colors, so they're preserved — an element with
`fill="none"` stays unfilled. References like `fill="url(#gradient)"` are also
left in place, though the gradient's own `stop-color` values *are* recolored.
Note the monochrome field overrides the color map whenever it's non-empty.

</details>

<details>
<summary>Will the output change anything besides colors?</summary>

Only matched color values are rewritten (in canonical lowercase hex like
`#ff0000`). Path data, geometry, structure, whitespace and every other
attribute pass through byte-for-byte, and the whole rewrite happens locally in
your browser — the SVG never leaves your machine.

</details>

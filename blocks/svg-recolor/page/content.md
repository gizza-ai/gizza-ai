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

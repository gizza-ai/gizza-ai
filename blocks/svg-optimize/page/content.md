## About this tool

**SVG Optimizer** shrinks and cleans up SVG markup for faster delivery and tidier
source, without ever changing how the image looks:

- **Collapses whitespace** — indentation and newlines between tags are removed and
  runs of whitespace inside tags are normalized.
- **Removes comments** — `<!-- … -->` blocks are stripped (leave *Remove comments*
  off to keep them).
- **Strips the XML prolog** — the `<?xml … ?>` declaration and `<!DOCTYPE …>` are
  removed; they're not needed for inline or web SVG.
- **Drops editor metadata** — `<metadata>` blocks and Inkscape/Sodipodi elements
  and attributes (`inkscape:*`, `sodipodi:*`, and their unused `xmlns`
  declarations) are removed.

Optional, off by default:

- **Remove id & class attributes** — handy for icons, but skip it if your CSS, JS
  or `<use>` references them.
- **Remove root width/height** — only applied when the `<svg>` has a `viewBox`, so
  the graphic scales responsively without losing its aspect ratio.

It's intentionally **safe**:

- **Path data and numbers are never rewritten**, so the rendered image is
  identical — this is not a lossy compressor.
- **`<script>`, `<style>` and `<text>`** keep their contents **verbatim**, so
  CSS, scripts and visible text aren't broken.
- Attribute *values* are never touched.

Everything runs **locally in your browser** via WebAssembly — your SVG is never
uploaded.

## FAQ

<details>
<summary>Will optimizing change how my icon looks?</summary>

No. This is a **lossless** cleaner: path `d` data, coordinates and numbers are
never rewritten or rounded, so the rendered image is byte-for-byte identical
in appearance. It only strips safe-to-remove cruft like comments, editor
metadata and structural whitespace.

</details>

<details>
<summary>Why are id/class and width/height left in by default?</summary>

Both removals can break real usage, so they're **off by default**. Deleting
`id`/`class` breaks any CSS, JavaScript or `<use xlink:href="#…">` that
references them. Removing the root `width`/`height` only happens when a
`viewBox` is present (so the graphic still scales), but some layouts rely on
the intrinsic size — enable each only when you know it's safe.

</details>

<details>
<summary>Does it touch my inline &lt;style&gt;, &lt;script&gt; or &lt;text&gt;?</summary>

No — the contents of `<style>`, `<script>` and `<text>` are kept **verbatim**.
Whitespace inside `<text>` can be visually significant, and collapsing CSS or
JS could break it, so those elements are passed through untouched. Attribute
values everywhere are also left alone.

</details>

<details>
<summary>What exactly gets stripped as "editor metadata"?</summary>

With **Remove editor metadata** on (the default), `<metadata>`, RDF blocks and
Inkscape/Sodipodi elements are dropped, along with their attributes
(`inkscape:*`, `sodipodi:*`) and the now-unused `xmlns:inkscape`,
`xmlns:sodipodi`, `xmlns:rdf`, `xmlns:cc`, `xmlns:dc` declarations. The core
`xmlns` needed to render the SVG is always kept.

</details>

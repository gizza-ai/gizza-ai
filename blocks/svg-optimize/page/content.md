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

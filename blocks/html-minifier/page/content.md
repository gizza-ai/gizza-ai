## About this tool

**HTML Minifier** shrinks HTML for faster delivery by:

- **Collapsing whitespace** — indentation and newlines between tags are removed,
  and runs of whitespace inside text are collapsed to a single space.
- **Removing comments** — `<!-- … -->` blocks are stripped (tick *Remove
  comments*; leave it off to keep them).
- **Normalizing tag whitespace** — extra spaces between attributes are collapsed.

It's built to be safe:

- **Significant inline spacing is preserved** — a real space between inline
  elements (e.g. `<b>a</b> <b>b</b>`) is kept, so words don't run together.
- **`<pre>`, `<textarea>`, `<script>` and `<style>`** keep their contents
  **verbatim**, so code and whitespace-sensitive blocks aren't broken.
- Attribute *values* are never touched.

Everything runs **locally in your browser** via WebAssembly — your HTML is never
uploaded. The reverse tool is **HTML Formatter** (pretty-print).

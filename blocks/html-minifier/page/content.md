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

## FAQ

<details>
<summary>Will minifying make words run together?</summary>

No. A run of whitespace inside text collapses to a **single space**, never to
nothing, and a meaningful space between inline elements — like
`<b>bold</b> <i>italic</i>` — survives. Only the purely structural whitespace
(indentation and newlines between block tags) is removed outright.

</details>

<details>
<summary>Does it also minify the JavaScript and CSS inside my page?</summary>

Deliberately not. `<script>` and `<style>` contents are passed through
**verbatim**, because whitespace-collapsing rules that are safe for HTML can
break string literals, template literals or CSS `calc()` expressions. Run the
extracted code through a dedicated JS/CSS minifier if you need that too.

</details>

<details>
<summary>What happens to &lt;pre&gt; blocks and attribute values?</summary>

`<pre>` and `<textarea>` keep their exact original whitespace, so code samples
and form defaults render unchanged. Attribute **values** are never modified
either — only the extra spaces *between* attributes inside a tag are collapsed.

</details>

<details>
<summary>Can I keep my HTML comments?</summary>

Yes — untick **Remove comments**. That preserves every `<!-- … -->` block,
which you'll want if your page relies on comment-based markers (build-tool
injection points, IE conditional comments, license headers).

</details>

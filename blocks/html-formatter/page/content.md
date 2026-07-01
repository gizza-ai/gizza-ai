## About this tool

**HTML Formatter** pretty-prints (beautifies) HTML with clean, consistent
indentation — one element per line, with nested children indented. Paste minified
or messy markup and get a readable, diff-friendly version back.

It's a forgiving formatter built for real-world HTML:

- **Void elements** (`<br>`, `<img>`, `<input>`, `<meta>`, …) don't add a
  bogus indentation level.
- **Self-closing tags**, **comments**, and the **doctype** are handled.
- **Quoted attributes** are safe — a `>` inside `title="x > y"` won't confuse it.
- **`<pre>`, `<textarea>`, `<script>` and `<style>`** keep their contents
  **verbatim**, so whitespace-sensitive blocks and code aren't mangled.

Set the **indent** to your preferred number of spaces per level (0–8, default 2).

Everything runs **locally in your browser** via WebAssembly — your HTML is never
uploaded.

### Handy for

- Making minified HTML readable.
- Normalizing indentation before committing a template.
- Inspecting the structure of a snippet you pasted from elsewhere.

## FAQ

<details>
<summary>Will formatting break my &lt;pre&gt;, &lt;textarea&gt;, or inline scripts?</summary>

No. The contents of `<pre>`, `<textarea>`, `<script>`, and `<style>` are kept
**verbatim** — their inner whitespace and line breaks pass through untouched, and
only the surrounding tags are indented. So preformatted text, embedded JavaScript,
and CSS come out exactly as they went in.

</details>

<details>
<summary>Can I use tabs, or change how deep the indentation is?</summary>

The indent setting takes a number of **spaces per level, from 0 to 8** (default 2);
values above 8 are clamped. Tab indentation isn't supported — if your project uses
tabs, format with spaces and convert afterwards.

</details>

<details>
<summary>Does it validate or repair broken HTML?</summary>

No — it's a formatter, not a validator. It's deliberately forgiving: void elements
like `<br>` and `<img>` don't add indent levels, self-closing tags, comments and
the doctype are recognized, and a `>` inside a quoted attribute such as
`title="x > y"` won't confuse it. But it won't report unclosed tags or rewrite
invalid markup; what you paste is what gets re-indented.

</details>

<details>
<summary>Can it minify HTML instead of beautifying it?</summary>

Not really. Setting the indent to 0 removes the leading spaces but still puts each
element on its own line — the output is readable, not compact. For shipping-size
reduction, use a dedicated HTML minifier after formatting.

</details>

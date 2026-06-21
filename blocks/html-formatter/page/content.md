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

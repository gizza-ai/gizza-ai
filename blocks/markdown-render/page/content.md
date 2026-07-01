## Markdown to HTML

Paste Markdown and get clean, sanitized HTML back. The conversion runs locally in
your browser (WebAssembly) — nothing is uploaded.

### What it supports

- **Headings**, **bold**/*italic*, blockquotes, and horizontal rules
- **Links** and **images**
- **Lists** — ordered, unordered, and nested
- **Tables** (GitHub-flavored)
- **Fenced code blocks** with language hints (`` ```rust ``)
- **Task lists** — `- [x]` / `- [ ]`
- **Strikethrough** (`~~text~~`) and **footnotes**

### Safe by default

The rendered HTML is sanitized before you get it: `<script>` tags, inline event
handlers (`onclick`…), and dangerous URL schemes (`javascript:`) are stripped, so
the output is safe to embed in another page.

### Good for

- Previewing a README or docs page as HTML.
- Turning Markdown notes into HTML for a CMS, email, or static site.

### FAQ

<details>
<summary>Is my Markdown uploaded?</summary>

No — the renderer is compiled to WebAssembly and runs
entirely in your browser tab.

</details>

<details>
<summary>Which flavor of Markdown is this?</summary>

CommonMark plus the common GitHub-flavored
extensions: tables, task lists, strikethrough, and footnotes.

</details>

<details>
<summary>Can I pass raw HTML through, like a &lt;script&gt; or an onclick attribute?</summary>

No — every render is passed through an HTML sanitizer (ammonia) after conversion.
`<script>` tags, inline event handlers, and `javascript:` URLs are removed, while
the markup Markdown legitimately produces (including task-list checkboxes and the
`class`/`id` attributes on code blocks and footnotes) is kept.

</details>

<details>
<summary>Do fenced code blocks come out syntax-highlighted?</summary>

The HTML is not pre-highlighted, but the language hint is preserved: ` ```rust `
becomes `<code class="language-rust">`. Drop the output into a page with
highlight.js or Prism and the coloring works out of the box.

</details>

<details>
<summary>Why did my straight quotes turn into curly ones?</summary>

Smart punctuation is enabled: `"quotes"` become typographic quotes, `--` becomes
an en dash, and `...` becomes an ellipsis. If you need the literal characters,
escape them with a backslash in the source Markdown.

</details>

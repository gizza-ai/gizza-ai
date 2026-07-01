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

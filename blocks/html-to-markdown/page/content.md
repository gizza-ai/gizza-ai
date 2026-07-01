## HTML to Markdown

Paste an HTML fragment or a whole page body and get clean Markdown back. The
conversion runs locally in your browser (WebAssembly) — nothing is uploaded.

### What it preserves

- **Headings** (`<h1>`–`<h6>` → `#`…`######`)
- **Links** and **images**
- **Lists** — ordered and unordered, including nesting
- **Code** — inline `` `code` `` and fenced ``` blocks ```
- **Tables**, **blockquotes**, **bold**/*italic*, and horizontal rules

### Good for

- Cleaning up copied rich text or scraped page HTML into Markdown for docs,
  READMEs, or notes.
- Converting a CMS/WYSIWYG export into plain Markdown.

### FAQ

<details>
<summary>Is my HTML uploaded?</summary>

No — the converter is compiled to WebAssembly and runs
entirely in your browser tab.

</details>

<details>
<summary>Does it run scripts or fetch the page?</summary>

No. It only parses the HTML you paste;
it does not execute JavaScript or fetch remote URLs.

</details>

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

**Is my HTML uploaded?** No — the converter is compiled to WebAssembly and runs
entirely in your browser tab.

**Does it run scripts or fetch the page?** No. It only parses the HTML you paste;
it does not execute JavaScript or fetch remote URLs.

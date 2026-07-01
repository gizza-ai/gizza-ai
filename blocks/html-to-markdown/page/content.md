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

<details>
<summary>Do I have to paste a complete HTML document?</summary>

No — a fragment like a single `<table>` or a copied `<div>` works just as
well as a full page. Parsing uses html5ever, the same engine family browsers
use, so unclosed or slightly malformed tags are repaired the way a browser
would render them. The only hard error is empty input.

</details>

<details>
<summary>What happens to classes, ids, and inline styles?</summary>

They're dropped. Markdown has no way to express `class`, `id`, or `style`
attributes, so the converter keeps the *structure* Markdown can represent —
headings, links, images, lists, code, tables, blockquotes, emphasis — and
discards presentational attributes. If you need the styling, keep the HTML.

</details>

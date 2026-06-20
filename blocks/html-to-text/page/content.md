## HTML to text

Paste HTML and get clean, readable plain text back — every tag removed, but the
paragraph and list structure kept. It runs locally in your browser; nothing is
uploaded.

### What it does

- Removes all HTML tags and attributes.
- Keeps paragraph breaks and puts list items on their own lines.
- Decodes HTML entities (`&amp;` → `&`, `&lt;` → `<`, …).
- Outputs **plain** text — no Markdown markers like `#` or `**` (use the
  HTML-to-Markdown tool if you want Markdown).

### Good for

- Pulling the readable text out of an email, a web page, or a CMS export.
- Getting a quick word count or a clean copy of an article body.

### FAQ

**Is my HTML uploaded?** No — the converter is compiled to WebAssembly and runs
entirely in your browser tab.

**Does it run scripts or fetch the page?** No. It only parses the HTML you paste.

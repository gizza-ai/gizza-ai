## About this tool

HTML Sanitizer removes active and risky markup from pasted HTML while preserving useful document structure. Use it when content comes from a WYSIWYG editor, email, a CMS field, an import script, or a scrape and you need either safe HTML markup or clean visible text.

The sanitizer uses an allowlist: common formatting, headings, lists, tables, links, images, and semantic containers can remain, while scripts, stylesheets, iframes, SVG/math payloads, forms, embeds, event-handler attributes, unsafe URL schemes, and unknown active tags are removed. It runs in your browser and does not upload the HTML.

Worked example:

Input:

```html
<p onclick="alert(1)">Hello <a href="javascript:alert(1)">world</a></p><script>steal()</script>
```

Safe HTML output:

```html
<p>Hello <a>world</a></p>
```

Choose **Plain text** when you want visible copy only. Turn off links, images, classes/IDs, comments, or inline styles when preparing lean CMS-safe snippets.

## Limits and edge cases

- This tool is a conservative sanitizer for snippets and documents, not a full browser-grade HTML parser.
- `<script>`, `<style>`, embeds, frames, SVG, MathML, forms, media tags, and head-only tags are removed with their contents where appropriate.
- Inline `style` is off by default. When enabled, obvious script vectors such as `javascript:`, `expression()`, and unsafe `url(data:text...)` are still removed.
- Safe URL schemes include common web/contact schemes and relative URLs; suspicious schemes are dropped.
- Plain-text mode first sanitizes the HTML, then extracts visible text, so removed script/style content does not leak into the result.

## FAQ

<details>
<summary>Does this make arbitrary user HTML completely safe to render?</summary>

It removes common XSS vectors and risky tags with an allowlist, which is appropriate for cleaning snippets and reducing attack surface. If you are accepting untrusted HTML in a production application, also enforce server-side sanitization, content security policy, and framework-specific escaping.

</details>

<details>
<summary>What is the difference between safe HTML and plain text?</summary>

Safe HTML preserves allowed tags such as paragraphs, headings, lists, tables, links, images, and inline formatting after unsafe parts are removed. Plain text removes all markup after sanitization and returns the visible text.

</details>

<details>
<summary>Why are my classes, IDs, images, links, or styles missing?</summary>

Those controls are configurable. Disable classes/IDs for lean pasted markup, disable images or links when you do not want external references, and enable inline styles only when you need safe style attributes. Unsafe URL schemes and dangerous style values are removed even when the related option is on.

</details>

<details>
<summary>Does the tool upload my HTML?</summary>

No. The sanitizer runs locally in the browser page through WebAssembly. The CLI version also runs locally and returns the sanitized text directly.

</details>

## About this tool

Marketing and transactional emails are usually sent as HTML, but you often need the
**plain-text version** — the `text/plain` alternative part that non-HTML clients show, the
copy you paste into a ticket, or a clean read of a newsletter without the layout. This tool
takes an **HTML email body** (the whole `<html>` document or just a `<body>` fragment) and
returns readable plain text: tags removed, HTML entities like `&amp;` and `&nbsp;` decoded,
and paragraphs, headings, and list items kept on their own lines.

The email-specific part is how it treats **links** and **line width**:

- **Links — Inline** (default): `<a href="https://example.com">click here</a>` becomes
  `click here (https://example.com)`, so the destination survives in plain text.
- **Links — Footnotes:** each link is numbered inline (`click here[1]`) and the URLs are
  collected into a `[1] https://example.com` reference block at the bottom — tidy for
  link-heavy newsletters.
- **Links — Text only:** keeps just the visible link text and drops the URL.
- **Wrap width:** set a column count (72 is the classic plain-text-email width) to hard-wrap
  long paragraphs on word boundaries. URLs and other long tokens are never split. Leave it at
  `0` for unwrapped output.

`mailto:` and `tel:` links are shown without the scheme (`email us (hi@example.com)`), and
in-page `#anchors`, `javascript:`, and `data:` links are dropped as noise.

### Worked example

Input (`links` = Footnotes):

```html
<p>See the <a href="https://example.com/docs">docs</a> and
<a href="https://example.com/pricing">pricing</a>.</p>
```

Output:

```
See the docs[1] and pricing[2].

[1] https://example.com/docs
[2] https://example.com/pricing
```

Everything runs locally in your browser via WebAssembly — the email content is never
uploaded, so it is safe to paste sensitive messages.

## FAQ

<details>
<summary>What's the difference between this and a plain "HTML to text" stripper?</summary>

A bare stripper just removes tags and usually discards link URLs. This tool is aimed at
emails: it keeps hyperlink destinations (inline or as numbered footnotes), can hard-wrap to
the classic 72-column plain-text-email width, strips `mailto:`/`tel:` schemes, and drops
in-page/`javascript:` links that add no value in plain text.

</details>

<details>
<summary>Should I paste the full HTML email or just the body?</summary>

Either works. You can paste the entire `<html>…</html>` source (for example the "view
source" of a message) or just the inner `<body>` fragment. Surrounding `<head>`, `<style>`,
and `<script>` content is ignored and does not appear in the output.

</details>

<details>
<summary>How are links rendered?</summary>

Choose one of three modes. **Inline** (the default) writes the link text followed by the URL
in parentheses. **Footnotes** numbers each link (`text[1]`) and lists the URLs at the bottom.
**Text only** keeps the visible text and drops the URL. When the visible text already *is* the
URL or email address, it is not duplicated.

</details>

<details>
<summary>Why would I wrap at 72 columns?</summary>

Plain-text email convention wraps body lines at around 72 columns so the message reads well in
terminals and quotes cleanly in replies. Set the wrap slider to 72 (or 78) to get that; set it
to 0 to leave lines unwrapped for on-screen reading. Long words such as URLs are never broken,
even if they exceed the wrap width.

</details>

<details>
<summary>Are images, tables, and buttons preserved?</summary>

Images are dropped (plain text has no images), and their `alt` text is not currently emitted.
Tables and button-styled links are flattened to their text content — a button that links
somewhere is treated like any other link. If you need the visual layout, keep the HTML
version; this tool produces a text reading of the content.

</details>

## Limits & notes

- Output is plain text only — no Markdown markers (`#`, `**`, `-`) are added.
- Image `alt` text and CID/attachment images are not included.
- The wrap width accepts 0–200 columns; 0 disables wrapping.
- Runs fully in-browser; nothing is uploaded.

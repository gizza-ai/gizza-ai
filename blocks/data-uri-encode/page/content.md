## About this tool

The **data URI encoder** turns text into a self-contained `data:` URI (RFC 2397)
that embeds the content directly — no separate file or network request needed.
A data URI looks like `data:<mime>;base64,<payload>` and can be dropped straight
into a CSS `url(...)`, an `<img src>`, an `<a href>`, a JSON field, or an HTML
email.

Paste your text, pick a **MIME type** (for example `text/plain`, `text/html`,
`image/svg+xml`, or `application/json`), and choose an **encoding**:

- **Base64** — compact and safe for any content; produces `data:<mime>;base64,…`.
- **URL** — percent-encodes the text into `data:<mime>,…`, which stays readable
  for short ASCII snippets.

Everything runs locally in your browser — your text never leaves your machine,
and the tool works offline with no sign-up.

### Tips

- For inline SVG icons, use `image/svg+xml` so the markup renders as an image.
- A `charset` is allowed in the MIME field, e.g. `text/html;charset=utf-8`.
- To go the other way and read a `data:` URI, use the data URI decoder. To turn
  a whole file into a data URI, use the file-to-data-URI tool.

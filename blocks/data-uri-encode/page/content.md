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

## FAQ

<details>
<summary>Should I choose Base64 or URL encoding?</summary>

Base64 (the default) is the safe choice — it handles any content, including binary-ish
text and Unicode, and produces `data:<mime>;base64,…`. URL (percent) encoding keeps
short ASCII snippets human-readable in the address, but non-ASCII characters balloon
into `%XX` sequences, so it's best for small plain-text payloads.

</details>

<details>
<summary>Can I include a charset in the MIME type?</summary>

Yes. The MIME field accepts parameters, so `text/html;charset=utf-8` or
`text/plain;charset=utf-8` works — useful when the consumer needs to know how to
decode the bytes. If you leave the field empty, `text/plain` is used.

</details>

<details>
<summary>Can this encode an image or other file?</summary>

This tool encodes **text** you paste. For a PNG, font, or any other file on disk,
use the file-to-data-URI tool, which reads the file bytes directly. For inline SVG
you can paste the SVG markup here with the `image/svg+xml` MIME type.

</details>

<details>
<summary>Is there a size limit on data URIs?</summary>

The tool itself doesn't cap the output (it reports the URI length so you can judge),
but browsers do have practical limits — very large data URIs slow down parsing and
some contexts (like CSS in older browsers) truncate them. Keep inline assets in the
tens of kilobytes.

</details>

## About this tool

A **data URI** (also called a data URL) embeds a small file directly inside a
URL instead of pointing at one. It has the form
`data:[<mediatype>][;base64],<data>` — for example
`data:text/plain;base64,SGVsbG8=` or
`data:image/png;base64,iVBORw0KGgo...`. They show up in CSS `background-image`
rules, inline `<img src>` tags, email HTML, JSON payloads, and copy-pasted
screenshots.

This decoder reverses one. Paste a `data:` URI and it splits out:

- the **MIME type** (defaulting to `text/plain` when the URI omits it);
- any **media-type parameters**, such as `charset`;
- the **encoding** — Base64 or percent-encoded (URL) text;
- the **decoded size** in bytes; and
- the **payload** itself: printable content is shown as decoded text, while
  binary content (images, PDFs, fonts, …) is reported with its file type
  detected from the magic bytes plus a hex preview.

It tolerates whitespace and line breaks inside the Base64, accepts the bare
`data:,...` short form, and preserves commas that appear inside the data.

Everything runs locally in your browser via WebAssembly — the data URI you paste
is never uploaded to a server.

### Examples

- `data:text/plain;charset=utf-8,Hello%20World` → text `Hello World`
- `data:text/html;base64,PGgxPkhpITwvaDE+` → the HTML `<h1>Hi!</h1>`
- `data:image/png;base64,iVBORw0KGgo=` → detected file type `image/png`

## FAQ

<details>
<summary>My data URI decodes to an image — can I see or save the picture?</summary>

Binary payloads aren't rendered here; instead you get the file type sniffed from the magic bytes (e.g. `image/png`), the decoded byte size, and a hex preview of the first 64 bytes. To view the image itself, paste the whole URI into your browser's address bar or an `<img src>` attribute.

</details>

<details>
<summary>Why does it say the charset is US-ASCII when I never specified one?</summary>

That's the RFC 2397 rule: a data URI with no media type (or a bare `data:,...`) defaults to `text/plain;charset=US-ASCII`, so the decoder reports that default explicitly rather than leaving the field blank.

</details>

<details>
<summary>The Base64 I copied has line breaks in it — will that break decoding?</summary>

No. Whitespace and newlines inside the Base64 payload are stripped before decoding, which is handy for URIs copied out of wrapped CSS or email HTML. Commas *after* the first one are preserved as data, so percent-encoded text containing commas decodes intact.

</details>

<details>
<summary>Is there a size limit on what it can decode?</summary>

Decoding happens fully in your browser, so nothing is uploaded. Text output is truncated at 100,000 characters and the binary hex preview shows the first 64 bytes — the reported decoded size and a `truncated` flag always reflect the full payload.

</details>

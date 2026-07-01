## About this tool

**EML Parser** reads a raw `.eml` (RFC 5322 / RFC 822) email message and breaks
it into its parts:

- **Headers** — subject, date (normalized to ISO-8601), message-id, and the
  `From`, `To`, and `Cc` addresses with display names.
- **Bodies** — the decoded **plain-text** and **HTML** bodies. MIME
  `Content-Transfer-Encoding` (base64, quoted-printable) and RFC 2047 encoded
  headers are decoded automatically.
- **Attachments** — each attachment's filename, MIME content-type, and size in
  bytes.

Paste the full raw message (everything from the `From:` line down, including all
MIME parts). Everything is parsed **locally in your browser** via WebAssembly —
your email is never uploaded.

### Where to get the raw .eml

- In most desktop mail clients: *File → Save As… → .eml*, or "Show original" /
  "View source".
- In Gmail: open the message → ⋮ menu → **Download message** (`.eml`).

### Common uses

- Inspect headers and routing without opening the mail in a client.
- Pull out attachment names and types from a saved message.
- Debug MIME structure and encoding issues.

## FAQ

<details>
<summary>Can I extract the attachment files themselves?</summary>

Not with this tool — it reports each attachment's **filename, MIME content-type,
and size in bytes**, but doesn't export the file contents. To get the actual
files, save them from your mail client; use this parser when you only need to see
*what* a message carries.

</details>

<details>
<summary>Why does the parser show no body (or no HTML body)?</summary>

Usually because only part of the message was pasted. The input must be the **full
raw source** — every header line and all MIME parts, from the first `From:` /
`Received:` header down to the final boundary. Text copied out of a mail client's
reading pane has no MIME structure and can't be parsed. The HTML body line also
only appears when the message actually contains a `text/html` part; plain-text-only
mail shows just the text body.

</details>

<details>
<summary>Do I need to decode base64 or =?UTF-8?...?= gibberish myself?</summary>

No. MIME `Content-Transfer-Encoding` (base64 and quoted-printable) is decoded
automatically for the bodies, and RFC 2047 encoded-words in headers — the
`=?UTF-8?B?...?=` form you see in subjects and display names — are decoded too.
Dates are also normalized to ISO-8601.

</details>

<details>
<summary>Is it safe to paste a private email here?</summary>

Yes — parsing happens locally in your browser via WebAssembly; the message is never
uploaded to a server. Still, if you're sharing the parsed output with someone else,
remember headers can contain IPs and internal hostnames.

</details>

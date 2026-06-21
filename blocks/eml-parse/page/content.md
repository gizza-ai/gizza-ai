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

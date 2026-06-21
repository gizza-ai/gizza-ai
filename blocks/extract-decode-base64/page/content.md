## About this tool

**Find & Decode Base64** scans a block of text, locates every embedded Base64
blob, and decodes it — so you don't have to copy each one into a decoder by hand.

For each blob it tells you:

- **Decoded text** — when the bytes are printable UTF-8.
- **File type + hex preview** — when the bytes are binary and match a known
  format (e.g. `image/png`, `application/pdf`), it shows the detected MIME type
  and the leading bytes in hex.

It understands both **standard** (`+`/`/`) and **URL-safe** (`-`/`_`) Base64, with
or without padding. Random alphanumeric runs that decode to neither printable
text nor a recognized file type are ignored, to cut down on false positives.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Inspecting JWTs, API tokens, and HTTP headers.
- Pulling readable content out of logs, JSON, or config dumps.
- Identifying what a `data:` URI or embedded blob actually contains.

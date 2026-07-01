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

## FAQ

<details>
<summary>Why wasn't my Base64 string detected?</summary>

Candidates must be at least **16 characters** long (about 12 decoded bytes),
use one alphabet consistently (a token mixing `+`/`/` with `-`/`_` is
rejected), and have a valid Base64 length. On top of that, the decoded bytes
must look like real content — at least ~85% printable text, or a recognized
file signature. Short tokens and blobs that decode to random bytes are
deliberately skipped to avoid false positives.

</details>

<details>
<summary>Does it handle URL-safe Base64 and missing padding?</summary>

Yes. Both the standard (`+`/`/`) and URL-safe (`-`/`_`) alphabets are decoded,
and trailing `=` padding is optional — the alphabet is picked per token based
on which marker characters it contains.

</details>

<details>
<summary>What do I get for a binary blob like an embedded image?</summary>

Instead of garbled text you get the detected file type (e.g. `image/png`,
`application/pdf`) sniffed from the magic bytes, the decoded byte count, and a
hex preview of the first 32 bytes. A binary blob whose format isn't recognized
is treated as noise and not reported.

</details>

<details>
<summary>Can I paste a whole JWT?</summary>

Yes — the dots between segments split the token naturally, so the header and
payload decode as readable JSON (they're URL-safe Base64). The signature
segment is random-looking bytes with no known file signature, so it's usually
filtered out rather than shown as garbage.

</details>

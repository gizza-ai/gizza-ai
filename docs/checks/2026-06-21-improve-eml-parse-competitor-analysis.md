# eml-parse — competitor analysis & differentiation

**Tool:** `gizza-ai/eml-parse` — parse a raw `.eml` / RFC 822 message into
structured headers, decoded text/HTML bodies, and an attachment list.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| Online "EML viewer / reader" sites (encryptomatic, mailviewer, etc.) | Web | Most **upload the .eml to a server** to render it — bad for confidential mail. Many are ad-heavy and render HTML bodies live (tracking-pixel / script risk). |
| `formail`, `munpack`, `ripmime` | CLI | Powerful but Unix-only, fiddly flags, and split across several tools (headers vs MIME extraction); not structured JSON. |
| Python `email` / Node `mailparser` | Library | Need to write code; encoding/MIME edge cases are easy to mishandle. |
| Desktop mail clients | App | Heavyweight; "view source" shows raw text, not a structured breakdown. |

## How gizza's tool is better / different

1. **Runs locally — mail never leaves the device.** Chat service worker, CLI, or
   browser page, all in WASM. The opposite of the upload-to-render web viewers.
2. **Structured output.** One call returns subject, ISO-8601 date, message-id,
   `From`/`To`/`Cc` (name + address), the decoded plain-text and HTML bodies, and
   every attachment's filename / MIME type / byte size — as clean JSON (chat/CLI)
   or a readable summary (page).
3. **Decoding done for you.** MIME multipart, base64 / quoted-printable transfer
   encodings, and RFC 2047 encoded headers are decoded automatically (verified:
   a base64 attachment body decoded to its real 9-byte length).
4. **Safe by construction.** The page shows the HTML body as *text*, so remote
   tracking pixels and scripts never load — unlike live-rendering web viewers.
5. **Three surfaces, one Rust core** (the robust `mail-parser` crate).

## Verification

The CLI was run on a multipart message with a base64-encoded `application/pdf`
attachment: headers, both bodies, and the attachment (name `doc.pdf`, type
`application/pdf`, decoded size 9 bytes) were all extracted correctly.

## Possible future enhancements

- Optional attachment **content** extraction (base64 data-URI per attachment) —
  would move it toward a file-output envelope.
- Surface additional headers (Reply-To, List-Unsubscribe, Received chain) and
  DKIM/SPF/Authentication-Results for deliverability debugging.

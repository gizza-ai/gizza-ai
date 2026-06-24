# data-uri-encode — competitor analysis (2026-06-23)

## Tool

`data-uri-encode` builds a self-contained `data:` URI (RFC 2397) from **text**,
with a chosen **MIME type** and either **Base64** or **percent (URL)** encoding.
Surfaces: chat skill, CLI, standalone page (`/tools/data-uri-encode/`). Pure
Rust (`base64` + hand-rolled percent encoder) → runs on every backend including
the chat Service Worker, and fully offline on the page.

Output (flat JSON / page string): `data_uri`, `mime`, `encoding`, `bytes`,
`uri_length`.

## Competitors surveyed (top 5)

1. **liminfo Data URL Encoder** — text/SVG/HTML/CSS/JSON → data: URI with the
   scheme, MIME type, optional `;base64` flag, encoded payload; decode mode too.
2. **peasydev Data URI Converter** — paste content, pick MIME type, get the full
   data: URI; separate decode mode.
3. **hidekazu-konishi Base64 Tool** — Base64 with "copy as Data URI"
   (`data:mime/type;base64,...`) for HTML/CSS embedding.
4. **base64encode.org / base64decode.org** — generic Base64 of text/files; a
   "uri" variant for URL-safe text; not data-URI-specific.
5. **elmah.io Base64 Image Encoder** — image file/URL → Base64 data URI (file
   oriented).

## Feature diff (fit-to-model)

| Capability | Competitors | data-uri-encode | Status |
|---|---|---|---|
| Build `data:<mime>;base64,…` from text | yes | yes (default) | covered |
| Percent/URL-encoded form `data:<mime>,…` | some | yes (`encoding=url`) | covered |
| Choose MIME type | yes | yes (free-text, validated) | covered |
| MIME parameters (e.g. `;charset=utf-8`) | partial | yes (preserved) | covered |
| Default MIME = `text/plain` | yes | yes | covered |
| Report byte count / URI length | rare | yes (`bytes`, `uri_length`) | covered (edge) |
| Client-side / offline / no upload | yes | yes (pure Rust, runs in browser) | covered |
| Invalid-MIME guardrail | rare | yes (rejects non `type/subtype`, stray `data:`/commas/whitespace) | covered (edge) |
| Decode a data: URI | yes (paired) | separate tool `data-uri-decode` | covered (sibling) |
| **Encode an uploaded file** | yes | **out of model for this tool** — handled by the sibling `file-to-data-uri` (file/url source) | n/a |

## In-model gaps closed

None outstanding. The standard data-URI-encoder feature set (scheme + MIME +
`;base64` flag + payload, both base64 and URL encoding, client-side) is fully
covered. The tool additionally validates the MIME type and reports byte/URI
length, which most competitors omit.

## Out-of-model / intentionally separate (not built)

- **File upload → data URI.** A binary-file input is a different I/O shape and is
  already provided by `blocks/file-to-data-uri` (url/ref source). Keeping
  `data-uri-encode` text-only is deliberate so it gets a real text-input page;
  duplicating file encoding here would just shadow `file-to-data-uri`.
- **Decoding.** Provided by `blocks/data-uri-decode` (the documented round-trip
  partner). Listed in the skill description so the LLM routes correctly.

No competitor copy, branding, or trademarks were used.

Sources:
- [liminfo Data URL Encoder](https://www.liminfo.com/tools/dataurl)
- [peasydev Data URI Converter](https://peasydev.com/dev/data-uri/)
- [hidekazu-konishi Base64 Tool](https://hidekazu-konishi.com/tools/base64_encoder_decoder_tool.html)
- [base64encode.org (uri variant)](https://www.base64encode.org/enc/uri/)
- [elmah.io Base64 Image Encoder](https://elmah.io/tools/base64-image-encoder/)
- [MDN — data: URLs](https://developer.mozilla.org/en-US/docs/Web/URI/Reference/Schemes/data)

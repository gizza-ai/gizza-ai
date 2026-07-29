# base64url-converter — competitor analysis (2026-07-29)

Scan done BEFORE implementing. One WebSearch ("base64 to base64url converter
online tool URL-safe") + fetch of the three most relevant competitor pages. All
notes are paraphrased — no competitor copy, branding, or trademarks reproduced.

## Competitors reviewed

1. **octalone.com — Base64 URL Converter** — "Convert standard Base64 to URL-safe
   format (replace +/ with -_) and remove padding." Browser-local, no upload.
   Page copy is thin; direction/padding controls not exposed in the fetched text.
2. **elysiatools.com — Base64URL Encoder** — offers four operations in one
   dropdown: Encode to Base64URL, Decode from Base64URL, **Standard → Base64URL**,
   **Base64URL → Standard**. Extra "Output encoding" select (UTF-8 text / Hex)
   for the decode-to-bytes path. Textarea + operation select + optional output
   format + Run button.
3. **devtoollab.com — Base64URL Encoder/Decoder** — encode/decode text ⇄
   Base64url with a **"Keep padding" checkbox**, **"Whitespace cleanup for pasted
   tokens"**, a "Load Example" button, Clear/Reset, and history. FAQ framed around
   JWT segments, OAuth tokens, URL-safe API payloads.

(base64.guru and several others in the SERP are text→Base64url *encoders*, i.e.
the encode-bytes family, not standard⇄url transcoders.)

## Table-stakes (params / defaults / UX)

| Feature | Seen at | In our model? | Decision |
| --- | --- | --- | --- |
| Standard → Base64url (swap +/ → -_) | all | yes | `direction=to-url` |
| Base64url → standard (swap -_ → +/) | elysia | yes | `direction=to-standard` |
| Auto-detect direction | (implicit) | yes | `direction=auto` (default) — our differentiator |
| Keep / strip `=` padding toggle | devtoollab, octalone | yes | `padding=auto\|keep\|strip` (3-way, richer than a checkbox) |
| Whitespace cleanup for pasted tokens | devtoollab | yes | always strip ASCII whitespace |
| Validate that input is real Base64 | (implicit) | yes | `validate` boolean |
| Load-example / worked examples | devtoollab | yes | 3 `[[example]]` chips + examples table |
| Browser-local, no upload, no sign-up | all | yes | native to gizza |

## Out-of-model / considered, not built

- **Encode plain text → Base64url** (elysia/devtoollab/base64.guru): that is the
  *encode-bytes* family (multi-encoder / base-decoder already cover Base64), a
  different tool. This tool is deliberately a pure alphabet+padding transcoder,
  per the backlog definition ("swapping +/ for -_ and handling padding"). Listed,
  not built — keeps scope crisp and avoids duplicating `multi-encoder`.
- **Decode Base64url → UTF-8 text / Hex output** (elysia "Output encoding"):
  same reason — decoding to bytes is the decoder family's job.
- **History / recently-used list** (devtoollab): stateful/account-ish UX; gizza
  pages are stateless. Not built.
- **File upload**: out of the pure-compute page model for a text transform.

## Decisions

Ship a focused transcoder with three enum/boolean controls that cover every
transcoding table-stake, plus an `auto` direction (our edge — none of the
competitors auto-detect) and a 3-way padding policy (richer than the common
keep-padding checkbox). Whitespace is always ignored so pasted/wrapped tokens
"just work." Text-encode and decode-to-bytes are explicitly deferred to the
existing encoder/decoder tools rather than bloating this one.

## About this tool

LZ-String is a compression algorithm designed for the browser. Instead of
producing raw bytes, it packs its output into characters that survive the places
web apps actually need to store data — URL query strings, `localStorage`,
`sessionStorage`, cookies and JSON — so you can stash a lot of state without a
backend.

This tool runs the algorithm entirely client-side (compiled to WebAssembly) and
produces output that is **byte-compatible with pieroxy's original `lz-string`
JavaScript library**, so payloads created here decompress in your app and vice
versa.

### Output formats

- **Base64** — portable ASCII using `A–Z a–z 0–9 + /` with `=` padding. Safe in
  JSON, headers, and anywhere plain text is expected. Matches
  `LZString.compressToBase64`.
- **URL-safe** — the encoded-URI-component alphabet (`+` and `/` become `-` and
  `_`, no `=` padding). Drops straight into a `?param=` value with no further
  `encodeURIComponent`. Matches `LZString.compressToEncodedURIComponent`.
- **UTF-16** — packs 15 bits into each UTF-16 character. The most compact choice
  for `localStorage`, which stores UTF-16 and would otherwise waste ~⅓ of the
  space on Base64 text. Matches `LZString.compressToUTF16`.

### How to use

1. Paste your text and pick **Compress** (the default) to shrink it, or
   **Decompress** to restore an existing payload.
2. Choose the **format** — when decompressing, this must match the format the
   payload was created with.
3. The result appears instantly. Compression is most effective on repetitive or
   structured text (JSON, logs, config); very short or already-random strings
   may not shrink.

Everything happens in your browser — your text is never uploaded.

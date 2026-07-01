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

## FAQ

<details>
<summary>Can my JavaScript app decompress payloads made here (and vice versa)?</summary>

Yes. Each format is byte-compatible with pieroxy's `lz-string`: **Base64**
matches `compressToBase64` (including `=` padding to a multiple of 4),
**URL-safe** matches `compressToEncodedURIComponent`, and **UTF-16** matches
`compressToUTF16`. The library's raw `compress()` format is deliberately not
offered — its unbalanced UTF-16 code units don't survive URLs or storage.

</details>

<details>
<summary>Why does decompressing say the input isn't a valid payload?</summary>

Almost always a format mismatch: a payload must be decompressed with the same
format it was compressed with (a `compressToEncodedURIComponent` string won't
decode as Base64). The tool also rejects input containing characters outside
the chosen format's alphabet up front, so a corrupted or truncated payload
fails loudly instead of silently decoding to an empty string.

</details>

<details>
<summary>Which format gives the smallest result?</summary>

For `localStorage`/`sessionStorage`, **UTF-16** — it packs 15 bits into every
stored character, while Base64 text wastes roughly a third of UTF-16 storage.
For URLs use **URL-safe**, which needs no further `encodeURIComponent`. For
JSON, headers, or anywhere plain ASCII is expected, **Base64** is the safe
default.

</details>

<details>
<summary>Why did my text get bigger after compressing?</summary>

LZ-String is a dictionary (LZW-style) compressor: it wins on repetitive,
structured text like JSON, logs, and config. A very short string or
already-random data has little for the dictionary to reuse, and the encoding
overhead can make the output longer than the input — that's expected, not a
bug.

</details>

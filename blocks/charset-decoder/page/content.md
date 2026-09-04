## About this tool

Use this decoder when you have bytes but not text: a hex dump copied from a packet capture, a
base64 value from an API payload, or a legacy file snippet whose charset is not UTF-8. Paste the
bytes, choose `auto` or an explicit encoding label, and the page decodes locally without uploading
the input.

Example: `48 65 6c 6c 6f 2c 20 77 6f 72 6c 64 21` with `input_format=hex` and `charset=utf-8`
returns `Hello, world!`. For Cyrillic legacy bytes, `cff0e8e2e5f2` with
`charset=windows-1251` returns `Привет`; `output=compare` shows how the same bytes look under
common alternatives such as KOI8-R, Shift_JIS, GBK and Big5.

Limits and edge cases: pasted input is capped at 1 MiB before decoding; the hexdump and compare
views preview the first 4096 bytes; automatic detection is a best effort and short samples can be
ambiguous, so choose an explicit charset when you know it. `per_line=true` is for one encoded value
per line and works with the text and escaped views only.

## FAQ

<details>
<summary>When should I use charset=auto?</summary>

Use `auto` when you do not know the encoding. It checks byte-order marks first, then ASCII,
then valid UTF-8, then a statistical detector. For tiny or repetitive samples, detection can be
wrong; re-run with an explicit charset if the output looks like mojibake.

</details>

<details>
<summary>What input formats are accepted?</summary>

Hex may include spaces, colons, dashes, commas, `0x` byte prefixes, `\\x` byte escapes or line
breaks. Base64 may be padded or unpadded, use the standard or URL-safe alphabet, contain line
breaks, or come from a `data:*;base64,` URI.

</details>

<details>
<summary>How do strict errors differ from replace?</summary>

`errors=replace` keeps decoding and inserts `�` for malformed byte sequences while reporting the
replacement count. `errors=strict` stops at the first invalid byte offset, which is useful when you
need to validate that a dump really belongs to the selected charset.

</details>

<details>
<summary>Is this the same as transcoding a file?</summary>

No. This page is optimized for pasted byte snippets and text output. Whole-file byte-to-byte
conversion, large uploads, or saving a file in a new encoding belong in a file conversion tool.

</details>

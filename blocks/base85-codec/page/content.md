## What this tool does

Encode text or hex bytes to **Base85**, or decode a Base85 string back to text or
hex. Base85 packs four bytes into five printable characters, making it more
compact than hex and often shorter than Base64 while staying copy-and-paste
friendly.

Everything runs locally in WebAssembly. Your input is not uploaded.

## Supported variants

| Variant | Best for | Notes |
| --- | --- | --- |
| **Ascii85** | Adobe/PostScript/PDF-style Base85 | Uses the `!` through `u` alphabet, supports the `z` all-zero shortcut, allows partial final groups, and can wrap encoded output in `<~ ... ~>`. |
| **Z85** | ZeroMQ / fixed-block binary tokens | Uses the ZeroMQ alphabet. Input bytes must be a multiple of 4 when encoding, and encoded strings must be a multiple of 5 when decoding. |
| **RFC 1924** | Python `base64.b85encode` compatibility | Uses the RFC 1924 alphabet and supports partial final groups. |

## Data formats

- **text** (default): encode UTF-8 text, or decode bytes back to UTF-8 text.
- **hex**: encode raw bytes written as hex (`48 65 6c` or `0x48656c`), or decode
  Base85 back to hex. Use this for binary data or outputs that are not valid
  UTF-8.

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `Hello World!` | encode · ascii85 · text | `87cURD]i,"Ebo80` |
| `87cURD]i,"Ebo80` | decode · ascii85 · text | `Hello World!` |
| `864fd26fb559f75b` | encode · z85 · hex | `HelloWorld` |
| `Man ` | encode · rfc1924 · text | ``O<`^z`` |

## Tips

- Turn on **Adobe frame** when you need Ascii85 output wrapped as `<~...~>` for
  tools that expect Adobe-style delimiters.
- Z85 is strict by design: use hex mode and make sure the byte count is divisible
  by 4 before encoding.
- If decoding to text fails, switch **Data format** to **hex** to inspect the raw
  bytes.

## FAQ

<details>
<summary>Which variant matches Python's base64.b85encode?</summary>

The **RFC 1924** variant. Python's `b85encode`/`b85decode` use the RFC 1924 alphabet, so pick that variant to round-trip with Python. `a85encode` output matches the **Ascii85** variant instead.

</details>

<details>
<summary>Why does Z85 reject my input?</summary>

Z85 is defined only for whole 4-byte groups: encoding requires the byte count to be a multiple of 4, and decoding requires the string length (ignoring whitespace) to be a multiple of 5. If your data isn't block-aligned, pad it or switch to Ascii85 or RFC 1924, which accept any length.

</details>

<details>
<summary>Do I need to strip the &lt;~ ... ~&gt; wrapper before decoding?</summary>

No. When decoding Ascii85, the tool strips a leading `<~` and trailing `~>` automatically, and it ignores whitespace and line breaks inside the string. The **Adobe frame** option only affects encoding, where it wraps the output in those delimiters.

</details>

<details>
<summary>Decoding says the result is not valid UTF-8 — what now?</summary>

The decoded bytes are binary rather than text. Switch **Data format** to **hex** and decode again: you'll get the exact bytes as a hex string instead of a UTF-8 error.

</details>

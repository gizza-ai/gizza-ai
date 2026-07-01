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

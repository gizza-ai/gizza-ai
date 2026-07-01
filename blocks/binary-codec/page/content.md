## What this tool does

Convert any text to a **binary** bit string, or decode a binary string back to the
original text — instantly, right in your browser. Every byte becomes eight `0`/`1`
bits. Nothing is sent to a server: it runs locally, works offline, and needs no
sign-up. Pick a **Mode**, then optionally choose a **byte delimiter** or a
per-byte `0b` **prefix**.

## Modes

| Mode | What it does |
| --- | --- |
| **encode** (default) | Turns your text into binary — e.g. `Hi` becomes `01001000 01101001` (each byte is eight bits). |
| **decode** | Reverses it — turns a binary string back into the original text. Decoding ignores whitespace, delimiters, and the `0b` prefix. |

## Formatting options (encode)

| Option | What it does | Example |
| --- | --- | --- |
| **Byte delimiter** | Inserts a separator between bytes — `space` (default), `none`, `colon`, `dash`, `comma`, or `newline`. | `Hi` · colon → `01001000:01101001` |
| **Per-byte prefix** | Adds `0b` before each byte. | `Hi` · `0b` + space → `0b01001000 0b01101001` |

## Decode output — text or raw bytes

| Format | When decoding |
| --- | --- |
| **text** (default) | Renders the bytes as UTF-8 text (errors if they aren't valid UTF-8) |
| **bytes** | Shows the decoded bytes as a plain lowercase hex byte string — use this for binary data that isn't readable text |

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `Hi` | encode | `01001000 01101001` |
| `Hello` | encode · no delimiter | `0100100001100101011011000110110001101111` |
| `Hi` | encode · `0b` prefix · space | `0b01001000 0b01101001` |
| `01001000 01101001` | decode | `Hi` |
| `01001000:01100101:01101100` | decode | `Hel` |

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and it keeps
working offline once the page has loaded.

**What encoding does it use for text?** Text is read and written as **UTF-8**, so
each character becomes its one-or-more UTF-8 bytes, and each byte becomes eight
bits. An emoji or accented letter therefore spans several bytes (`é` →
`11000011 10101001`).

**Can I paste binary with spaces or `0b` prefixes?** Yes. Decoding ignores ASCII
whitespace, the `:` `-` `,` delimiters, and the `0b` prefix, so a string copied
from a hex dump, a debugger, or Python's `bin()` output decodes without editing.

**Does the bit count matter?** Yes — after stripping formatting, the number of
`0`/`1` bits must be a multiple of 8 (one byte per eight bits). A partial byte is
an error.

**My decoded output looks garbled.** The bytes probably aren't UTF-8 text. Switch
the **Decode output** to **bytes** to see the raw hex instead.

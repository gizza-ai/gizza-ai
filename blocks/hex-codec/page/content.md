## What this tool does

Convert any text to a **hexadecimal** string, or decode a hex string back to the
original text — instantly, right in your browser. Nothing is sent to a server: it
runs locally, works offline, and needs no sign-up. Pick a **Mode**, then optionally
choose a **byte delimiter**, **uppercase** digits, or a **per-byte prefix**.

## Modes

| Mode | What it does |
| --- | --- |
| **encode** (default) | Turns your text into hex — e.g. `Hi` becomes `4869` (each byte is two hex digits). |
| **decode** | Reverses it — turns a hex string back into the original text. Decoding is case-insensitive and ignores whitespace, delimiters, and `0x` / `\x` prefixes. |

## Formatting options (encode)

| Option | What it does | Example |
| --- | --- | --- |
| **Byte delimiter** | Inserts a separator between bytes — `none` (default), `space`, `colon`, `dash`, `comma`, or `newline`. | `Hi` · colon → `48:69` |
| **Uppercase hex** | Emits uppercase `A–F` digits instead of lowercase. | `é` → `C3A9` |
| **Per-byte prefix** | Adds `0x` or `\x` before each byte. | `Hi` · `0x` + space → `0x48 0x69` |

Mix and match freely — e.g. a space delimiter plus the `0x` prefix gives the
classic `0x48 0x69 0x21` C-array style.

## Decode output — text or raw bytes

| Format | When decoding |
| --- | --- |
| **text** (default) | Renders the bytes as UTF-8 text (errors if they aren't valid UTF-8) |
| **bytes** | Shows the decoded bytes as a plain lowercase hex byte string — use this for binary data that isn't readable text |

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `Hi` | encode | `4869` |
| `Hello` | encode · space delimiter | `48 65 6c 6c 6f` |
| `Hello` | encode · colon · uppercase | `48:65:6C:6C:6F` |
| `Hi` | encode · `0x` prefix · space | `0x48 0x69` |
| `4869` | decode | `Hi` |
| `48:65:6c 6c 6f` | decode | `Hello` |
| `c3a9` | decode | `é` |

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your input never leaves your device, and it keeps
working offline once the page has loaded.

</details>

<details>
<summary>What encoding does it use for text?</summary>

Text is read and written as **UTF-8**, so
accented letters and emoji become their multi-byte UTF-8 sequences (`é` → `c3a9`).

</details>

<details>
<summary>Can I paste hex with spaces or colons?</summary>

Yes. Decoding ignores ASCII whitespace,
the `:` `-` `,` delimiters, and `0x` / `\x` prefixes, so a string copied from a hex
dump, a `0x`-prefixed C array, or a `\x`-escaped string all decode without editing.

</details>

<details>
<summary>My decoded output looks garbled.</summary>

The bytes probably aren't UTF-8 text. Switch
the **Decode output** to **bytes** to see the raw hex instead.

</details>

<details>
<summary>Lowercase or uppercase?</summary>

Lowercase is the default and the most common in tools
and protocols. Toggle **Uppercase hex** when a spec or system expects `A–F` caps.

</details>

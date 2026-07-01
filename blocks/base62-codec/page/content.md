## What this tool does

Encode any text, hex bytes, or a decimal number to **Base62**, or decode a
Base62 string back to the original data — instantly, right in your browser.
Nothing is sent to a server: it runs locally, works offline, and needs no
sign-up. Pick a **Mode**, an **Alphabet**, and a **Data format**.

## What is Base62?

Base62 represents data using the 62 **alphanumeric** characters — the digits
`0-9`, the uppercase letters `A-Z`, and the lowercase letters `a-z`. Because it
uses no padding (`=`) and none of the extra symbols that Base64 relies on
(`+`, `/`), a Base62 string is compact and safe to drop straight into a URL, a
filename, or a database key. That's why it's the go-to encoding for **short IDs
and URL slugs** — think the string in a shortened link.

## Modes

| Mode | What it does |
| --- | --- |
| **encode** (default) | Turns your input into a Base62 string — e.g. `Hello World!` becomes `T8dgcjRGkZ3aysdN`. |
| **decode** | Reverses it — turns a Base62 string back into the original text, bytes, or number. |

## Data format — text, hex, or number

| Format | When encoding | When decoding |
| --- | --- | --- |
| **text** (default) | Reads the input as UTF-8 text | Renders the bytes as UTF-8 text (errors if they aren't valid UTF-8) |
| **hex** | Reads the input as a hex byte string (`48 65 6c` or `0x48656c`) | Renders the decoded bytes as hex — use this for binary data |
| **number** | Reads the input as a non-negative decimal integer of **any size** | Renders the value back as a decimal integer |

Use **number** when you want to shorten an integer ID — for example turning the
database row `12345` into the slug `3D7`, or a 128-bit counter into a handful of
characters. Numbers are handled at **arbitrary precision**, so values far larger
than 64 or 128 bits round-trip exactly.

Use **hex** whenever your data is binary and not readable text — for example a
raw hash or a random token.

## Alphabets

Base62 uses all 62 alphanumerics; the two common variants differ only in the
order of the letters:

| Alphabet | Order | Notes |
| --- | --- | --- |
| **standard** (default) | `0-9 A-Z a-z` | Digits, then uppercase, then lowercase — the GMP order used by most short-ID libraries. |
| **inverted** | `0-9 a-z A-Z` | Digits, then lowercase, then uppercase. |

Both variants have **no padding**. For byte input, each leading `0x00` byte is
preserved as a leading `0` in the output; for **number** input there is no
leading-zero padding (the integer `0` encodes to `0`).

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `Hello World!` | encode · standard · text | `T8dgcjRGkZ3aysdN` |
| `T8dgcjRGkZ3aysdN` | decode · standard · text | `Hello World!` |
| `12345` | encode · standard · number | `3D7` |
| `4294967295` | encode · standard · number | `4gfFC3` |
| `516b6fcd0f` | encode · standard · hex | `69hruW7` |

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**What's the difference from Base64?** Base64 uses 64 symbols including `+`, `/`,
and `=` padding, which aren't URL-safe without extra escaping. Base62 sticks to
the 62 letters and digits, so its output needs no padding and is safe in URLs,
filenames, and IDs — at the cost of being slightly longer than Base64.

**How is Base62 different from Base58?** Base58 drops the four visually
ambiguous characters `0 O I l` (leaving 58), which makes it easier to read and
type by hand. Base62 keeps all 62 alphanumerics, so it's a bit more compact but
not optimised for hand-copying.

**Which alphabet should I pick?** Use **standard** (`0-9A-Za-z`) unless a
specific library you're matching uses the inverted order. The two are not
interchangeable — a string encoded with one alphabet must be decoded with the
same one.

**Can I shorten a big number?** Yes — set the **Data format** to **number** and
paste the integer. There's no size limit; huge counters and UUID-sized values
round-trip exactly.

**My decoded output looks garbled.** The bytes probably aren't UTF-8 text.
Switch the **Data format** to **hex** to see the raw bytes, or **number** if the
value is an integer.

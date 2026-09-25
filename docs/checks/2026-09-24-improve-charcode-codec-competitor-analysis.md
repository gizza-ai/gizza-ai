# charcode-codec — competitor analysis (2026-09-24)

Backlog row: `charcode-codec` — "Converts text to a list of Unicode/ASCII character codes (in a chosen base) and reconstructs text from a list of codes." Type hint: pure.

All competitor observations are paraphrased. No competitor copy, branding, or trademarked wording is reused in the tool.

## Dup check

Searched existing blocks for char/code/unicode/ascii conversion surfaces. Nearby tools handle URL encoding, base conversion, text encoding, and hashing, but none provide a bidirectional text ⇄ character-code codec with Unicode scalar/UTF-8/UTF-16/ASCII scopes and base selection. Build is distinct.

## Competitors scanned

1. Coddy ASCII Converter (`coddy.tech/tools/ascii-converter`)
2. Tool Lab Text to ASCII Code Converter (`zeroglabs.dev/en/tools/textToAscii`)
3. Randomly Text to ASCII Codes / Char Code Converter (`randomly.online/text-tools/convert-encode/text-to-ascii-codes`)

## Table-stakes and design decisions

| Capability | Seen in | Fit | Decision |
| --- | --- | --- | --- |
| Convert text to numeric character codes | all | in-model | `mode=encode` |
| Convert numeric codes back to text | Coddy, Randomly | in-model | `mode=decode` |
| Decimal output | all | in-model | `base=dec` default |
| Hex output | all | in-model | `base=hex` with uppercase toggle |
| Binary output | Coddy, Tool Lab | in-model | `base=bin` |
| Octal output | Tool Lab | in-model | `base=oct` |
| ASCII-focused mode with non-ASCII rejection | all ASCII tools | in-model | `scope=ascii` |
| Unicode/code-point support for non-ASCII text | Coddy, Unicode tools | in-model | `scope=unicode-scalar` default |
| Byte-oriented view | developer-facing converters | in-model | `scope=utf8-bytes` |
| UTF-16 code-unit view | Unicode/debug converters | in-model | `scope=utf16-units` |
| Separator control | common UX | in-model | `delimiter=space|comma|newline|none` |
| Prefix/escape notation (`0x`, `\x`, `U+`) | Unicode/escape tools | in-model | `prefix=none|0x|\x|U+`, decoder strips common prefixes |
| Padded fixed-width output | Unicode/debug tools | in-model | `padding=fixed` |
| JSON/machine-readable output | uncommon | in-model differentiator | `format=json` |
| Client-side/local conversion | all browser tools advertise | in-model | pure Rust/WASM, no network |

## Out-of-model / deliberately not built

- HTML entity named-reference tables (`&amp;`, `&copy;`) and JavaScript/JSON escape parsing are separate escaping tools, not raw character-code conversion.
- Charset transcoding (Shift_JIS, ISO-8859-1, UTF-16 files) belongs to the existing text-encoding-converter family; this tool works on the already-decoded input string.
- Font glyph IDs, keyboard scan codes, and Morse code are different code systems.

## Descriptor as built

| Param | Type | Default | Notes |
| --- | --- | --- | --- |
| `input` | string required | — | text to encode or codes to decode |
| `mode` | enum `encode|decode` | `encode` | bidirectional |
| `base` | enum `dec|hex|bin|oct` | `dec` | chosen numeric base |
| `scope` | enum `unicode-scalar|utf8-bytes|utf16-units|ascii` | `unicode-scalar` | what one code counts |
| `delimiter` | enum `space|comma|newline|none` | `space` | encode separator; decoder is tolerant |
| `prefix` | enum `none|0x|\x|U+` | `none` | encode prefix; decoder strips common prefixes |
| `padding` | enum `none|fixed` | `none` | zero-pad to scope width |
| `uppercase` | boolean | `true` | hex digit case on encode |
| `format` | enum `text|json` | `text` | result shape |

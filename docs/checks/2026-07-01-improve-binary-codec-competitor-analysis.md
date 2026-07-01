# Competitor analysis: binary-codec

Date: 2026-07-01
Tool: `binary-codec`

## Competitors reviewed

| competitor | useful capabilities observed | gaps considered |
| --- | --- | --- |
| BinaryTranslator.com | Text↔binary plus decimal/octal/hex base conversions; download/share of output. | Core text↔binary is implemented; cross-base numeric conversion lives in separate gizza tools; download/share are page-infra, out of the compute model. |
| dCode Binary Code Converter | Educational decode of binary with tolerant spacing, ASCII/UTF-8 handling, and byte grouping. | Whitespace/delimiter-tolerant decode and UTF-8 byte handling are implemented; site-specific tutorial copy was not copied. |
| cryptii Binary decoder | Composable pipe with configurable byte grouping/separators and alphabet. | Byte delimiter selection is implemented; full pipe/recipe chaining is out of a single-tool model. |
| LingoJam Binary Encoder & Decoder | One-box instant encode/decode with a plain UTF-8 mapping. | Instant per-surface encode/decode is implemented across chat, CLI, and page. |
| ConvertText.app Binary Translator | Text→binary and binary→text with space-separated byte groups and copy output. | Space-delimited default matches the common convention; copy-to-clipboard is page-infra. |

## In-model gaps closed / confirmed present

- Encode text (read as UTF-8) to a per-byte 8-bit binary string; decode binary back to text.
- Selectable byte delimiter: `space` (default), `none`, `colon`, `dash`, `comma`, `newline` — matching the range competitors expose.
- Optional per-byte `0b` prefix on encode; decode strips `0b` automatically.
- Tolerant decode: ignores ASCII whitespace, the common delimiters (`: - ,`), and the `0b` prefix, so any emitted form round-trips — parity with the "paste binary with spaces" behaviour competitors advertise.
- UTF-8 correctness: multi-byte characters (accents, emoji) encode/decode to their full UTF-8 byte sequences (`é` → `11000011 10101001`).
- Non-UTF-8 handling: `format = "bytes"` renders decoded bytes as a plain lowercase hex string instead of erroring, covering binary that isn't printable text.
- Validation errors for bad mode/format/delimiter/prefix, non-multiple-of-8 bit counts, and non-UTF-8 text decodes.
- Three verified surfaces: chat skill (schema drift-guarded), CLI smoke vectors, and the browser page with query-param deep links.

## Out-of-model or deferred gaps

- Decimal/octal/hex numeric base conversion — covered by separate gizza tools, not folded into this one.
- Download / copy-to-clipboard / social-share buttons — page-infrastructure UX, not part of the compute model.
- Recipe/pipe chaining (cryptii/CyberChef style) — not a single-tool capability.

## Verification notes

- `cargo test --workspace`: 13 core/schema tests pass, including the chat-schema drift guard.
- `wafer build` validates the chat `block.wasm` (297.8 KiB); `wasm-pack` builds the page wasm.
- CLI smoke: encode (`Hi` → `0100100001101001`), delimiter+`0b` prefix, decode (`01001000 01101001` → `Hi`), and `format=bytes` (`… → deadbeef`).
- Playwright: 5 page tests — default encode, decode, delimiter/prefix, non-UTF-8 hex, and query-param deep link.

Original analysis only; no competitor copy, branding, or assets were copied.

Sources: [BinaryTranslator.com](https://www.binarytranslator.com/), [dCode](https://www.dcode.fr/binary-code), [cryptii](https://cryptii.com/pipes/binary-decoder/), [LingoJam](https://lingojam.com/BinaryEncoder%26Decoder), [ConvertText.app](https://converttext.app/en/tools/binary-translator/)

# Competitor analysis: base62-codec

Date: 2026-07-01
Tool: `base62-codec`

## Competitors reviewed

| competitor | useful capabilities observed | gaps considered |
| --- | --- | --- |
| Base64.sh Base62 Encoder Decoder | Presents Base62 as URL-safe text/number/data conversion with simple encode/decode actions. | Text and number conversion are in-model and implemented; file/data upload workflows are deferred. |
| Better Converter Base62 Encode | Simple online Base62 text encoder aimed at quick copy/paste conversion. | Core text encoding is covered; gizza adds decode, hex bytes and arbitrary-size integer modes. |
| Encode64 Base62 Decoder | Focuses on decoding with configurable alphabet/character-set options and validation. | Alphabet selection is in-model and implemented with standard and inverted variants; broader custom alphabets are deferred to avoid ambiguous CLI/page schema. |
| MinifyTool URL Minifier | Uses Base62-style short, memorable URL/slug generation as a user-facing workflow. | URL-shortener hosting and redirects require a server and are out of model; local number-to-slug conversion is implemented. |
| DevToys Web Base62 tool | Developer-tool workflow for Base62 encode/decode as part of a larger utilities suite. | Quick local encode/decode is covered; gizza differentiates with CLI/chat/page parity and binary hex handling. |

## In-model gaps closed

- Added Base62 encode and decode for UTF-8 text.
- Added `hex` format for binary byte input/output, including preservation of leading zero bytes.
- Added `number` format for arbitrary-precision non-negative decimal integers, useful for short IDs and URL slugs.
- Added both common alphabet orders: `standard` (`0-9A-Za-z`) and `inverted` (`0-9a-zA-Z`).
- Added validation and actionable errors for invalid mode, alphabet, format, hex, number and Base62 characters.
- Added page copy covering Base62 vs Base64/Base58, alphabets, examples and privacy.
- Added Playwright coverage for defaults, decode, number/inverted variants, hex leading zeros and query-param deep links.

## Out-of-model or deferred gaps

- Hosted URL shortening/redirects are not implemented because they require server-side state.
- File upload conversion is deferred; the chat/CLI/page schema is text-only and supports binary data through hex.
- Fully custom alphabets are deferred to keep the schema stable and avoid accidental duplicate or missing characters.

## Verification notes

- Unit tests cover standard vectors, hex vectors, number vectors, leading zero byte preservation, inverted alphabet behavior, defaults and error paths.
- Drift guard pins the chat/CLI schema: required `input`, plus optional `mode`, `variant` and `format` enums.
- Page tests verify encode/decode behavior across text, hex and number modes.

Original analysis only; no competitor copy, branding or assets were copied.

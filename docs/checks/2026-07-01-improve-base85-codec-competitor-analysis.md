# Competitor analysis: base85-codec

Date: 2026-07-01
Tool: `base85-codec`

## Competitors reviewed

| competitor | useful capabilities observed | gaps considered |
| --- | --- | --- |
| Browserling Base85/Ascii85 tools | Simple browser forms for encoding/decoding printable Base85 text. | Core encode/decode is implemented; gizza adds multiple variants and CLI/chat parity. |
| CyberChef Base85 operations | Rich developer workflow with Ascii85/Base85 operations composed in recipes. | Recipe chaining is out of model; variant selection and binary hex handling are in-model and implemented. |
| dCode Ascii85 Encoder/Decoder | Explains Adobe Ascii85, handles decode/encode, and surfaces framing-oriented usage. | Ascii85 plus Adobe `<~ ~>` framing is implemented; site-specific educational copy was not copied. |
| ZeroMQ Z85 reference tooling | Z85 tools focus on the strict ZeroMQ alphabet and block-size constraints. | Z85 alphabet and strict multiple-of-4/5 validation are implemented. |
| Python/RFC 1924 compatible b85 utilities | Developer references and snippets commonly distinguish Python `b85`/RFC 1924 from Adobe Ascii85. | RFC 1924 alphabet support is implemented; custom recipe/code generation is deferred. |

## In-model gaps closed

- Added Base85 encode/decode for text and hex byte strings.
- Added three practical variants: Ascii85, Z85, and RFC 1924.
- Added Ascii85 `z` zero-group shortcut and optional Adobe `<~...~>` framing on encode; decode strips framing automatically.
- Added strict Z85 validation for byte lengths divisible by 4 and encoded lengths divisible by 5.
- Added RFC 1924 alphabet compatibility with partial final groups.
- Added error handling for invalid variants, formats, malformed hex, invalid alphabet characters, invalid groups and non-UTF-8 decoded text.
- Added browser page copy, wafer fixtures, CLI smoke vectors, and Playwright page coverage including query-param prefill.

## Out-of-model or deferred gaps

- Full CyberChef-style recipe chaining is not part of a single gizza tool.
- File upload conversion is deferred; binary inputs are supported through hex to keep chat/CLI/page schemas identical.
- Fully custom alphabets are deferred because Base85 variants have non-trivial rules beyond just symbol order.

## Verification notes

- Unit tests cover Ascii85, Z85, RFC 1924, Adobe framing, partial groups, hex mode, whitespace tolerance and error paths.
- Drift guard pins the schema: required `input`, optional `mode`, `variant`, `format`, and `adobe_frame`.
- Playwright tests cover the page, variant selectors, checkbox behavior and query-param deep links.

Original analysis only; no competitor copy, branding or assets were copied.

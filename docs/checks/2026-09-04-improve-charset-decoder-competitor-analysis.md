# charset-decoder competitor analysis — 2026-09-04

Tool: `charset-decoder` — decode pasted hex/base64 bytes under a chosen or detected character set.

## Sources skimmed

Search: "online charset decoder hex base64 detect encoding tool".

- Monocalc Text Encoding Detector: browser tool focused on raw bytes, hex input, encoding detection, ranked candidates and confidence-style diagnostics.
- DenCode Encoding & Decoding Online Tools: broad encoder/decoder suite with text transforms, base64/hex style inputs and simple copyable output.
- Base64/charset utilities surfaced by Base64 Tools / Base64 Guru style pages: base64-first workflows with explicit character-encoding choices and explanatory copy about why base64 bytes are not automatically text.

## Table-stakes and fit decisions

| Observed pattern | Decision for this tool |
| --- | --- |
| Paste text/bytes directly rather than uploading a full file | In model: multiline `input` field for pasted hex/base64 snippets. |
| Accept both hex and base64 byte representations | In model: `input_format=auto|hex|base64`, with tolerant hex separators and standard/URL-safe base64. |
| Explicit charset selection for legacy encodings | In model: `charset` accepts WHATWG labels plus UTF-32LE/BE; examples mention UTF-8, UTF-16, Windows code pages, KOI8-R, Shift_JIS, GBK, Big5 and EUC-KR. |
| Auto-detection | In model: BOM/ASCII/UTF-8 fast paths plus chardetng statistical fallback. No fake confidence score because chardetng does not expose one. |
| Show alternatives when detection is ambiguous | In model: `output=compare` renders common candidate charsets side by side. |
| Diagnostics such as byte count and invalid replacements | In model: decoded result includes `bytes`, `chars`, `charset`, `charset_source`, `input_format`, `replaced` and optional BOM; `output=report` renders them as text. |
| Strict vs forgiving malformed-byte handling | In model: `errors=replace|strict` with strict byte-offset errors. |
| Batch/log workflow | In model: `per_line=true` decodes each non-empty line independently for text/escaped output. |
| File upload / full file conversion | Out of model for this page: gizza already has file-oriented conversion tools; this tool stays paste-snippet focused to keep the page simple and memory-bounded. |
| Confidence scores | Out of model: the chosen detector has no confidence API, and inventing scores would mislead users. |

## UX requirements carried into the implementation

- Textarea for pasted byte dumps so line breaks survive.
- Select controls for fixed choices: input format, output view and error policy.
- Checkbox controls for BOM stripping and per-line decoding.
- Preset chips for a UTF-8 hex sample, a Windows-1251 sample and compare mode.
- Worked examples, limits, and FAQ copy explain ambiguity, accepted input formats and strict errors in original wording.

# base64-diff — competitor analysis (2026-08-16)

Scan run before finalising the block. Query used: `base64 compare diff decoded bytes online`. I reviewed representative browser and developer tools, then mapped their table-stakes features to the gizza model. Capability descriptions below are paraphrased; no competitor wording or branding is reused.

## Competitors reviewed

| # | Tool | Observed capability | Notes |
|---|------|---------------------|-------|
| 1 | CyberChef Base64 + Diff workflow | Decode from Base64, switch alphabets, then compare bytes/text in a recipe | Powerful but multi-step; users must manually build the decode-then-diff chain. |
| 2 | Online text compare tools with Base64 decode helpers | Decode two values then run a text diff | Good for UTF-8 payloads, weak for binary tokens and offsets. |
| 3 | Hex diff / binary compare utilities | Byte offsets, hex rows, changed ranges | Usually require uploaded files or already-decoded bytes, not pasted Base64 strings. |
| 4 | Base64 validators/decoders | Standard/Base64url decoding, padding repair, data-URI handling | They answer validity and decoded text, not "what changed between two payloads". |

## Table stakes → decisions

| Capability | Decision |
|---|---|
| Decode before comparing | Built. Both sides are decoded first, so padding, wrapping and alphabet spelling do not create false differences. |
| Standard and Base64url alphabets | Built. `alphabet=auto|standard|url`; auto detects per side and rejects mixed alphabets inside one side. |
| Lenient vs strict validation | Built. Lenient mode strips whitespace/data-URI prefixes and repairs padding; strict mode requires canonical RFC 4648 input. |
| Binary-safe byte diff | Built. Reports first offset, size delta, common prefix/suffix, SHA-256 and changed/added/removed byte ranges. |
| Hex dump view | Built. `output=hexdump` renders side-by-side hex + ASCII with differing rows marked. |
| Human-readable summary | Built. `output=summary` gives a compact verdict and one line per byte range. |
| Text diff for decoded UTF-8 | Built. `output=text` produces a unified line diff and rejects binary payloads with a clear message. |
| Shift-aware alignment | Built. `align=shift` trims common prefix/suffix for insertion/deletion-style changes; `align=offset` remains available for fixed-layout data. |
| Size/hash metadata | Built. JSON report includes decoded sizes, detected type and SHA-256 per side. |
| Upload-based file comparison | Out of model for this pasted-string pure block. File upload and multi-file binary diff are separate tool shapes; this one takes Base64 text fields. |
| Cryptographic meaning of changed bytes | Out of model. The tool reports payload differences; it does not infer token formats, decrypt payloads or validate signatures. |

## Not a duplicate

Existing `base64-validator`, `base64url-converter`, `base-decoder`, `hex-byte-inspector` and `diff-viewer` each cover one part of the workflow: decoding, validation, hex inspection or generic text diff. None accepts two Base64/Base64url strings and produces a byte-level decoded-payload diff with strict/lenient decoding, alphabet handling, shift alignment, hash metadata, hexdump and UTF-8 text diff outputs.

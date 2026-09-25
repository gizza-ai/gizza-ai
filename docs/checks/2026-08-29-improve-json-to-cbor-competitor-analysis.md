# json-to-cbor — competitor analysis (2026-08-29)

Scan run before implementation. Notes are paraphrased from public tool pages; no competitor copy, branding, or trademarks are copied into the tool page.

## Competitors reviewed

### 1. emn178 online-tools CBOR encode
- Encodes pasted JSON or CBOR diagnostic notation into CBOR bytes.
- Shows hex and Base64 output, with a file input path for local data.
- Table-stakes UX: large textarea, format selection, immediate error messages for invalid input.

### 2. OpenFormatter CBOR encoder
- Positions the tool as JSON to RFC 8949 CBOR bytes, browser-local.
- Outputs hex and Base64 and highlights size comparison between JSON text and CBOR bytes.
- Table-stakes UX: canonical/standard encoding expectations, copyable output, worked example.

### 3. Toolinix CBOR/JSON converter
- Bidirectional CBOR decode and encode workflow with hex and Base64 views.
- Includes inspect/viewer style output for debugging binary payloads.
- Table-stakes UX: input encoding selector on decode; clear parse errors.

### 4. Birdor CBOR converter
- Browser-local CBOR ⇄ JSON conversion.
- Shows both hex and Base64 output representations.
- Table-stakes UX: simple two-panel converter, examples, no server upload.

### 5. CoderTools CBOR encoder/decoder
- Converts between JSON and CBOR and advertises C-array style output for embedded workflows.
- Table-stakes UX: multiple output encodings and a practical IoT/debugging framing.

## Table stakes and decisions

| Table stake | Verdict | Where it landed |
| --- | --- | --- |
| Parse JSON into CBOR bytes | in-model | core RFC 8949 encoder over serde_json values |
| Hex output | in-model | `output=hex` default |
| Base64 output | in-model | `output=base64` |
| JSON wrapper with byte counts | in-model | `output=json` includes encoding, bytes, sizes |
| Diagnostic summary | in-model | `output=summary` gives hex plus length comparison |
| Canonical key ordering | in-model | `canonical=true` sorts object keys by encoded key bytes |
| Optional hex grouping | in-model | `group` inserts spaces every N bytes for readability |
| Byte-size comparison | in-model | all non-raw formats report JSON UTF-8 bytes vs CBOR bytes |
| Decode CBOR back to JSON | out of scope | separate `cbor-to-json` direction, not part of this slug |
| File upload / binary download | out of scope | current backlog row asks for pasted JSON to hex/base64 text; binary file output would be a separate file surface |
| CBOR diagnostic notation as input | out of scope | requires a diagnostic-notation parser; this tool intentionally accepts JSON only |
| C array output | in-model but omitted | useful for embedded users, but backlog asked only hex/base64; can be a future enum value |

## Design decisions

1. The encoder implements the JSON-compatible CBOR data model only: null, booleans, numbers, strings, arrays, and maps with string keys.
2. Integers are encoded as CBOR major type 0/1 when they fit exactly; non-integer numbers use IEEE-754 f64.
3. Canonical map sorting is on by default for reproducible bytes. JSON object keys are encoded as text strings and sorted by their encoded CBOR byte sequence.
4. Errors name the JSON parse failure and byte/line/column context from serde_json instead of returning partial bytes.

## About this tool

MessagePack is a compact binary representation for JSON-like values. This converter parses the
JSON you paste, serializes it locally, and renders the exact bytes in a copyable text form so you
can put them into tests, fixtures, API clients or binary protocols.

Example: `{ "a": 1, "b": 2 }` becomes the hex bytes `82a16101a16202`: `82` is a two-entry map,
`a1 61` is the key `a`, `01` is the integer value, and the same pattern follows for `b`. Use
`output=annotated` when you want that byte-by-byte explanation for your own payload.

Limits and edge cases: input is capped at 1,000,000 UTF-8 bytes and JSON nesting is capped at 64
levels. JSON has no byte-string, extension or timestamp type, so this encoder emits MessagePack
nil/bool/int/float/string/array/map values only. `key_order=input` preserves document order;
`key_order=sorted` is useful when reproducible bytes matter.

## FAQ

<details>
<summary>Which output format should I choose?</summary>

Use `hex` for fixtures and packet-level comparisons, `base64` for embedding bytes in JSON or text
APIs, `bytes` for decimal arrays in code, `annotated` for learning/debugging the wire format, and
`summary` or `json` when you need byte counts and size savings.

</details>

<details>
<summary>Does this preserve object key order?</summary>

Yes. `key_order=input` keeps the order in the JSON document. Switch to `key_order=sorted` when you
need deterministic MessagePack bytes from semantically identical objects whose keys may arrive in
different orders.

</details>

<details>
<summary>Can it encode every MessagePack type?</summary>

It encodes the types JSON can represent: nil, booleans, integers, floats, strings, arrays and maps.
MessagePack binary blobs, extension records and timestamps have no JSON type marker here, so they
are intentionally not invented by this converter.

</details>

<details>
<summary>What does the old string-header mode do?</summary>

Modern MessagePack has a compact `str8` header for 32–255 byte strings. `spec=old` avoids that
header for older pre-2013 decoders by using the wider string header instead; the decoded text value
is the same, but the bytes differ.

</details>

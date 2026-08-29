## About this tool

JSON to CBOR encodes a pasted JSON value into Concise Binary Object Representation bytes. It supports the JSON-compatible CBOR data model: null, booleans, integers, floating-point numbers, UTF-8 strings, arrays, and objects with string keys.

The default output is continuous lowercase hex, which is convenient for tests, protocol fixtures, and logs. You can switch to Base64 for transport, or to a summary that reports both hex and Base64 with JSON-vs-CBOR byte counts.

### Worked example

Input:

```json
{"b":2,"a":1}
```

With canonical key ordering enabled, the output is:

```text
a2616101616202
```

That is a CBOR map of two pairs, sorted by encoded key bytes: key `a` with value `1`, then key `b` with value `2`.

### Limits and edge cases

- Maximum input is 1,000,000 UTF-8 bytes.
- This tool accepts JSON, not CBOR diagnostic notation.
- Integers are encoded as compact CBOR integer major types when they fit exactly. Non-integer JSON numbers are encoded as IEEE-754 double precision floats.
- Decode workflows belong in the opposite `cbor-to-json` direction; this encoder intentionally returns text representations of the bytes.

## FAQ

<details>
<summary>What does canonical key order do?</summary>

When enabled, object keys are encoded as CBOR text strings and sorted by their encoded byte sequence before the map is written. This makes the same JSON object produce reproducible bytes even if input keys arrived in a different order.

</details>

<details>
<summary>Why is the output hex by default?</summary>

Hex is the easiest representation to compare in fixtures, protocol docs, and command-line tests. Choose Base64 when you need a shorter text representation for a binary payload.

</details>

<details>
<summary>Can this encode arbitrary CBOR tags or byte strings?</summary>

No. JSON has no native byte-string or tag types, so this tool encodes the JSON data model only. Use a dedicated CBOR diagnostic-notation encoder if you need tagged values or byte strings that are not represented as JSON strings.

</details>

<details>
<summary>Does the JSON get uploaded?</summary>

No. The browser page runs the WebAssembly encoder locally, and the CLI runs the same logic on your machine.

</details>

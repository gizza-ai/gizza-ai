## About this tool

Paste a [MessagePack](https://msgpack.org/) blob as **hex** or **base64** and this
tool decodes it into readable, pretty-printed JSON. MessagePack is a compact
binary serialization format used in caches (Redis), network protocols, config
blobs, and driver payloads — great for machines, hard to read by eye. This
decoder turns those bytes back into JSON you can inspect.

Field and element **order is preserved** (MessagePack maps are ordered), and full
**uint64 precision** is kept, so a 64-bit id is not silently rounded to a float.
Choose the indent, or set it to `0` to minify to one line. Everything runs
locally in your browser using WebAssembly; the bytes are never uploaded.

MessagePack has a few types JSON cannot hold natively, so they are shown by a
small, documented convention:

- Raw binary (`bin`) → a base64 (default) or hex **string** of the bytes.
- Extension (`ext`) → `{ "$ext": <type>, "data": "<base64|hex>" }`.
- The reserved **timestamp** extension (type -1) → an RFC 3339 / ISO 8601 UTC
  string.
- Non-string map keys (numbers, booleans, nil) → stringified keys.

### Worked example

The hex blob `82a7636f6d70616374c3a6736368656d6100` is the canonical MessagePack
sample `{ "compact": true, "schema": 0 }`. With **Input encoding** set to `hex`
and **Indent** set to `2`, the result is:

```json
{
  "compact": true,
  "schema": 0
}
```

Set **Indent** to `0` and the same input minifies to one line:

```json
{"compact":true,"schema":0}
```

### Limits and edge cases

- Input is a hex or base64 **string**, not a file upload. To decode a `.msgpack`
  file, base64- or hex-encode it first (for example `base64 file.msgpack`) and
  paste the text.
- Several MessagePack values concatenated back-to-back are returned as a JSON
  array, in order.
- Non-finite floats (NaN, Infinity) are rendered as `null`, since JSON has no way
  to represent them.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions. -->

<details>
<summary>What input formats does this accept?</summary>

Paste the MessagePack bytes as a **hex** or **base64** string. With **Input
encoding** left on `auto`, the tool detects which one you pasted; force it with
`hex` or `base64` if the encoding is ambiguous. Hex input may include spaces,
`:`, `-`, or `,` separators and `0x` markers — they are ignored — so byte dumps
paste cleanly.

</details>

<details>
<summary>How are binary and extension types handled?</summary>

JSON has no binary type, so raw `bin` values and unknown `ext` payloads are
encoded as a **string**. Use the **Binary / ext encoding** option to pick
`base64` (default) or `hex`. An unknown extension becomes
`{ "$ext": <type>, "data": "<encoded>" }`, and the reserved timestamp extension
(type -1) is decoded to an ISO 8601 UTC string like `2018-01-01T00:00:00Z`.

</details>

<details>
<summary>Does it keep full precision for large integers?</summary>

Yes. A 64-bit unsigned integer such as `18446744073709551615` (`u64::MAX`) is
preserved exactly instead of being rounded to a floating-point number. Map key
order is preserved too, so the JSON reflects the original document structure.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The decoder is compiled to WebAssembly and runs entirely in your browser
tab. The MessagePack bytes you paste never leave your device, and there is no
sign-up or request to a server.

</details>

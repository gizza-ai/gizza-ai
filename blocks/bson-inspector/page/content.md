## About this tool

Paste one BSON document as Base64 or hex and this tool decodes it into a readable,
ordered view. The default `tree` output shows every field name, BSON type, and
value, including nested documents and arrays. Switch to `json` when you need
canonical MongoDB Extended JSON v2 with `$oid`, `$date`, `$binary`,
`$numberInt`, `$numberLong`, `$numberDecimal`, `$timestamp`, and the other typed
wrappers preserved.

Use it for quick checks on `mongodump` fragments, driver test fixtures, or raw
binary payloads copied from logs. Hex input may include spaces, colons, or
dashes, so byte dumps are easy to paste. Everything runs locally in your browser;
the BSON bytes are not uploaded.

### Worked example

The Base64 sample `FgAAAAJoZWxsbwAGAAAAd29ybGQAAA==` is the BSON document
`{ "hello": "world" }`. With **Output format** set to `tree`, the result is:

```text
hello: String "world"
```

With **Output format** set to `json` and **Indent** set to `0`, the same input
renders as compact Extended JSON:

```json
{"hello":"world"}
```

### Limits and edge cases

- The input must be exactly one complete BSON document, not a concatenated
  stream of multiple documents.
- File upload is not part of this page; Base64-encode a `.bson` file or paste a
  hex dump instead.
- Decimal128 values are rendered as canonical decimal strings, while unsupported
  or unknown element type bytes return an explicit parse error with an offset.

## FAQ

<details>
<summary>What kind of BSON input should I paste?</summary>

Paste the bytes of a single BSON document as **Base64** or **hex**. Base64 is the
default because it is compact and common for binary blobs in logs. Hex is useful
when you copied bytes from a debugger; spaces, `:` separators, and `-` separators
are ignored.

</details>

<details>
<summary>What is the difference between tree output and JSON output?</summary>

`tree` is a human inspection view: every line shows the field name, the BSON type,
and a readable value, with optional byte offsets. `json` emits canonical MongoDB
Extended JSON v2, preserving types that normal JSON cannot represent, such as
ObjectId, DateTime, int64, binary subtypes, Decimal128, MinKey, and MaxKey.

</details>

<details>
<summary>Can this open a .bson file directly?</summary>

No direct file picker is included in this generic tool page. Convert the file to
Base64 (for example with `base64 file.bson`) or to a hex dump, then paste that
text. Parsing still happens locally in WebAssembly after the text is pasted.

</details>

<details>
<summary>Why did I get a document length or terminator error?</summary>

BSON stores a document length at byte 0 and ends each document with a NUL byte.
If the pasted bytes are truncated, include extra bytes, or represent multiple
concatenated documents, those structure checks fail. Copy exactly one complete
document and try again.

</details>

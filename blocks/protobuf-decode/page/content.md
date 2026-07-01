## Decode protobuf without the .proto file

Protocol Buffers is a compact binary format, and the bytes on the wire only
tell you each field's **number** and **wire type** — not its name or its
declared type. That is exactly enough to walk the structure even when you do
not have the original `.proto` schema. This tool parses the wire bytes you
paste (as **base64** or **hex**) and shows you the full field tree.

## What you get for each field

Because the schemaless wire format is ambiguous, every field is shown with all
of its plausible interpretations:

- **varint** (wire type 0) — shown as an unsigned int, a signed int, a
  zigzag-decoded `sint`, and (for `0`/`1`) a bool.
- **fixed32** (wire type 5) — as `uint32`, `int32`, and a 32-bit `float`.
- **fixed64** (wire type 1) — as `uint64`, `int64`, and a 64-bit `double`.
- **length-delimited** (wire type 2) — recursively decoded as a **nested
  message** when the bytes parse cleanly, and also shown as a UTF-8 string
  (when printable) and as raw hex bytes.

Pick **JSON** output for a structured tree you can feed to another tool, or
**text** for a quick indented outline.

## Common uses

- Reverse-engineer an undocumented gRPC or protobuf API response.
- Inspect a serialized message captured from network traffic.
- Sanity-check what your own encoder actually wrote to the wire.

Everything runs locally in your browser via WebAssembly — your bytes never
leave your machine.

## FAQ

<details>
<summary>Why does a single field show three or four different values?</summary>

Because the wire format doesn't carry the declared type. A varint could be an
unsigned int, a negative `int32`, a zigzag `sint`, or a bool — the bytes are
identical — so the decoder prints every plausible reading and lets you pick the
one that matches the API. The same goes for fixed32/fixed64 (int vs float) and
length-delimited fields (nested message vs string vs bytes).

</details>

<details>
<summary>Why is my string decoded as a nested message (or vice versa)?</summary>

A length-delimited field is recursed as a nested message whenever its bytes
happen to parse cleanly as protobuf — short ASCII strings sometimes do. The
decoder therefore *also* shows the UTF-8 string and raw hex readings alongside
the message tree, so the true interpretation is never hidden.

</details>

<details>
<summary>What inputs will fail to decode?</summary>

Hex with an odd number of digits, invalid base64 characters, messages using the
long-deprecated start/end **group** wire types (3/4), and nesting deeper than 64
levels all produce a clear error. A truncated buffer errors at the point where a
field's length runs past the end of the data.

</details>

<details>
<summary>Can I paste bytes captured from gRPC?</summary>

Yes, but strip the 5-byte gRPC message prefix first (1 compression flag byte +
4-byte big-endian length) — the decoder expects the bare protobuf message that
follows it. In practice: drop the first 10 hex characters of the frame, then
paste the rest.

</details>

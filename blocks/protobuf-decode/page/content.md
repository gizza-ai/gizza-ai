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

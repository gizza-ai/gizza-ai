## What this tool does

gRPC doesn't send a protobuf message as bare bytes. On the wire it wraps every
message in a tiny **length-prefixed frame**: a 1-byte *compressed flag*, a 4-byte
*big-endian length*, then exactly that many payload bytes. A single HTTP/2 DATA
body — or a streaming RPC — can carry several of these frames back to back.

Paste a captured stream (as **base64** or **hex**) and this tool walks it frame by
frame: it shows each frame's compressed flag and declared length, then decodes the
embedded protobuf payload into a **field-number / wire-type tree** — no `.proto`
schema required. Everything runs locally in your browser; nothing is uploaded.

## The gRPC frame layout

Each `Length-Prefixed-Message` is:

```
+--------------------+----------------------------+----------------------+
| Compressed-Flag    | Message-Length             | Message              |
| 1 byte (0 or 1)    | 4 bytes, big-endian uint32 | <Message-Length>     |
+--------------------+----------------------------+----------------------+
```

- **Compressed-Flag** — `0` means the payload is raw protobuf; `1` means it was
  compressed with the codec named in the `grpc-encoding` header (commonly gzip).
- **Message-Length** — the byte length of the payload that follows.
- **Message** — the protobuf bytes (when uncompressed).

Frames repeat until the bytes run out. This parser validates the framing, splits
the frames, and decodes each uncompressed payload.

## How the payload is decoded

The protobuf wire format is self-describing only at the level of *field numbers*
and *wire types* — without the original `.proto` you can't know whether a varint
was an `int32`, an `enum`, or a `bool`. So for every field the decoder reports the
field number, the raw wire type, and **every plausible interpretation**:

- **varint** → `uint`, `int`, zig-zag `sint`, and `bool` when the value is 0 or 1
- **fixed32** → `uint32`, `int32`, `float`
- **fixed64** → `uint64`, `int64`, `double`
- **length-delimited** → recursively parsed as a **nested message** when it parses
  cleanly, plus shown as a UTF-8 `string` and as raw `hex` bytes

Choose **JSON** for a structured tree you can drill into, or **text** for a compact
indented outline.

## Example

The hex stream `00 00 00 00 03 08 96 01` is one frame: flag `0`, length `3`,
payload `08 96 01` — which decodes to the protobuf message `{ field 1: 150 }`.
Concatenate another frame after it and both are split out and decoded
independently.

## Good for

- Reverse-engineering an undocumented gRPC API from a captured request/response
- Debugging a gRPC call from a Wireshark / Charles / browser-devtools byte dump
- Confirming the framing and message boundaries of a streaming RPC
- Inspecting a protobuf payload when you only have the bytes, not the schema

## Notes & limits

- **Compressed frames** (flag = 1) are reported but **not decompressed** — decode
  the payload after decompressing it with the codec from the `grpc-encoding`
  header (try the matching decompress tool, then the protobuf decoder).
- If a frame's payload **isn't valid protobuf**, the tool still surfaces the frame
  (flag, length, hex) and notes the decode error — the framing parsed, the payload
  just isn't protobuf.
- The bytes must start at a frame boundary. If you paste a bare protobuf message
  (no 5-byte prefix), use the dedicated protobuf decoder instead.

<details>
<summary>Is this the same as a protobuf decoder?</summary>
<p>No — a protobuf decoder expects a single bare message. gRPC adds the
length-prefixed framing layer (compressed flag + 4-byte length) on top, and a
stream can hold many messages. This tool peels off that framing first, then hands
each uncompressed payload to the protobuf decoder.</p>
</details>

<details>
<summary>Why are there multiple interpretations per field?</summary>
<p>Without the original <code>.proto</code> schema the wire bytes are ambiguous: a
varint could be an integer, a signed zig-zag value, an enum, or a bool. The
decoder shows every reading so you can pick the one that matches your API.</p>
</details>

<details>
<summary>Does anything get uploaded?</summary>
<p>No. The parser is compiled to WebAssembly and runs entirely in your browser —
the bytes you paste never leave your device.</p>
</details>

## What this tool does

A WebSocket message is carried on the wire as one or more **frames**, defined by
[RFC 6455](https://www.rfc-editor.org/rfc/rfc6455). Each frame packs a lot into its
first two bytes: a FIN bit, three reserved bits, a 4-bit opcode, a mask flag, and a
payload length — followed (when the frame is masked) by a 4-byte masking key and the
masked payload.

Paste a single frame as **base64** or **hex** and this tool decodes every header
field and **unmasks the payload** for you, showing it as both hex and — for text
frames — UTF-8 text. Everything runs locally in your browser; the bytes you paste
never leave your device.

## The WebSocket frame layout

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-------+-+-------------+-------------------------------+
|F|R|R|R| opcode|M| Payload len |    Extended payload length    |
|I|S|S|S|  (4)  |A|     (7)     |             (16/64)           |
|N|V|V|V|       |S|             |   (if payload len==126/127)   |
+-+-+-+-+-------+-+-------------+-------------------------------+
|     Masking-key (if MASK set) |          Payload Data ...     |
+-------------------------------+-------------------------------+
```

- **byte 0** — FIN (`0x80`, last fragment of a message), RSV1/RSV2/RSV3
  (`0x40`/`0x20`/`0x10`, reserved for extensions), and the **opcode** in the low
  nibble.
- **byte 1** — the **MASK** bit (`0x80`) plus a 7-bit base payload length. If that
  value is `126`, the real length is the next **2 bytes** (big-endian `uint16`); if
  it is `127`, the real length is the next **8 bytes** (big-endian `uint64`).
- **Masking key** — present only when MASK is set: 4 bytes the client used to mask
  the payload. The tool unmasks the payload as `payload[i] XOR key[i % 4]`.

## Opcodes

| opcode | meaning |
| ------ | ------- |
| `0x0`  | continuation |
| `0x1`  | text (UTF-8) |
| `0x2`  | binary |
| `0x8`  | close (2-byte status code + optional reason) |
| `0x9`  | ping |
| `0xA`  | pong |

Opcodes `0x3`–`0x7` and `0xB`–`0xF` are reserved.

## Example

The hex frame `81 05 48 65 6c 6c 6f` decodes to FIN = true, opcode `0x1` (text),
unmasked, payload length 5, payload `48 65 6c 6c 6f` = the text **"Hello"**.

A masked client frame for the same message — `81 85 37 fa 21 3d 7f 9f 4d 51 58` —
has the MASK bit set, masking key `37 fa 21 3d`, and unmasks back to **"Hello"**.

## Good for

- Debugging a WebSocket connection from a Wireshark / browser-devtools byte dump
- Reverse-engineering a `ws://` / `wss://` protocol when you only have the bytes
- Confirming whether a frame is a text, binary, close, ping, or pong control frame
- Unmasking a client→server frame to read the payload the client sent
- Learning how RFC 6455 framing and the masking key work

## Notes & limits

- Only the **first** frame is decoded. If you paste several concatenated frames, the
  tool reports how many trailing bytes were left undecoded.
- This decodes a single **data/control frame**, not the opening HTTP **handshake**
  (the `Upgrade: websocket` request/response is plain HTTP, not a frame).
- **Extensions** that use the RSV bits (e.g. `permessage-deflate` compression) are
  reported via the RSV flags but the payload is **not** decompressed.

<details>
<summary>Why is the client payload masked?</summary>

RFC 6455 requires every client→server frame to be masked with a random 4-byte key
to defend intermediary proxies against cache-poisoning attacks. Server→client frames
are never masked. This tool detects the MASK bit and unmasks the payload for you.

</details>

<details>
<summary>What do payload lengths 126 and 127 mean?</summary>

The 7-bit length field only reaches 125. The value 126 is a marker that the real
length follows as a 2-byte big-endian integer; 127 means it follows as an 8-byte
big-endian integer. The tool reads the extended length automatically.

</details>

<details>
<summary>Does anything get uploaded?</summary>

No. The parser is compiled to WebAssembly and runs entirely in your browser — the
bytes you paste never leave your device.

</details>

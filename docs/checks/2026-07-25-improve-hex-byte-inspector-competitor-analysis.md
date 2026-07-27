# hex-byte-inspector — competitor analysis (2026-07-25)

Tool function: given a value expressed as hex (or base64/text), report its **byte
length, bit length and hex-char count**, show it converted between **hex / raw bytes
/ text / base64**, group the hex for readability, and note which **common
cryptographic value sizes** match that byte length (hash digests, AES/ChaCha keys,
Ed25519/secp256k1 keys & signatures, IVs). Browser-local, no server, no sign-up.

Scan of the real tools in this space (paraphrased — no copy/branding/trademarks
reproduced). Unreachable ones were replaced so the top-3 are all live.

## Competitors skimmed

1. **OnlineToolz — "Hex Count"** (onlinetoolz.ai/hex/hex-count). Counts **bytes,
   nibbles and characters** in a pasted hex string, in-browser, no sign-up. Table-
   stakes: byte count, nibble/hex-char count, tolerant of whitespace in the paste.
   No bit count, no crypto-size interpretation, no format conversion.

2. **ethproductions "bytes" — all-in-one byte counter** (ethproductions.github.io/bytes).
   Counts **bytes of a string under a chosen text encoding** (UTF-8/UTF-16/…), shows
   the running byte length live. Table-stakes: byte length of text input, encoding
   choice. We cover the UTF-8 case via `input_format=text`.

3. **CyberChef** (gchq.github.io/CyberChef). The reference "cyber Swiss-army knife":
   **From Hex / To Hex / From Base64 / To Base64 / decode text**, all client-side,
   nothing leaves the machine. Table-stakes for us: convert **between hex, base64 and
   text** and show the raw bytes. CyberChef is a general pipeline; ours is a focused
   one-shot *inspector* that also reports lengths and crypto-size hints in one view.

Supporting reference material (not a tool, informs the crypto-size table):
Wikipedia "Key size", learnmeabitcoin hash-function notes, KeyCDN/hashlib docs —
byte↔bit relationships and the canonical digest/key widths (MD5 16 B, SHA-1 20 B,
SHA-256 32 B, SHA-512 64 B, AES-128/192/256 = 16/24/32 B, Ed25519 sig 64 B,
secp256k1 compressed/uncompressed pubkey 33/65 B, AES-GCM/ChaCha20 nonce 12 B).

## Table-stakes → decision

| Feature | Seen in | In/out-of-model | Where it lands |
| --- | --- | --- | --- |
| Byte count | 1,2 | in | `Bytes:` line (always) |
| Nibble / hex-char count | 1 | in | `Hex chars:` line (always) |
| Bit count | refs | in | `Bits:` line (always) |
| Tolerant hex paste (spaces, `:` `-` `0x` `\x`) | 1 | in | hex parser strips them |
| Byte length of text under an encoding | 2 | in (UTF-8) | `input_format=text` |
| Convert hex ↔ base64 ↔ text | 3 | in | Hex/Base64/Text lines |
| Show raw bytes / grouped hex | 1,3 | in | grouped hex + `group_size` |
| Uppercase hex option | 1,3 | in | `uppercase` checkbox |
| Crypto-size interpretation (key/hash/sig hints) | refs | in | `Matches:` block + `interpret` toggle |
| Preset example values (a SHA-256, an AES-128 key) | — | in | `[[example]]` chips |
| Live keystroke recount | 1,2 | in (page runs on input) | page auto-runs per field change |
| File upload → inspect a whole file's bytes | — | **out** | listed below (page takes a text field, not a file) |
| Interactive hex-editor grid (click a byte to edit) | CyberChef-ish | **out** | listed below (bespoke stateful UI, not a one-shot compute) |
| Save/share a named workspace of steps | 3 | **out** | listed below (needs accounts/storage) |

Every table-stake above is either in the descriptor (byte/bit/char counts, format
conversion, grouping, uppercase, crypto interpretation, example chips) or listed
here as out-of-model (file upload, interactive editor grid, saved workspaces).
Nothing dropped silently.

## Out-of-model (considered, not built)

- **File-upload byte inspection** — the page surface is a text field; inspecting an
  arbitrary uploaded file's bytes would need a file input + the ffmpeg/media path,
  out of scope for a pure text tool.
- **Interactive hex-editor grid** — click-to-edit byte cells is a bespoke stateful
  widget, not the one-shot input→report compute this generator renders.
- **Saved / shareable workspaces** — needs accounts or server storage; gizza tools
  are stateless and browser-local.

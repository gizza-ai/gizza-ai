## About this tool

**Bencode decoder** converts between **bencode** and **JSON** in both directions.
Bencode is the compact serialization format used throughout **BitTorrent** — it
encodes the metainfo inside every `.torrent` file and the messages exchanged with
trackers and the DHT.

Bencode has just four types:

- **Integers** — `i42e`
- **Byte strings** — `4:spam` (a length, a colon, then exactly that many bytes)
- **Lists** — `l4:spami42ee`
- **Dictionaries** — `d3:cow3:moo4:spam4:eggse` (keys are byte strings, stored in
  sorted order)

### Modes

- **Decode** parses bencode text into readable JSON.
- **Encode** serializes JSON back into **canonical** bencode — dictionary keys are
  re-sorted by byte value, exactly as the BitTorrent spec requires.

### Binary data

A torrent's `pieces` field is a run of raw 20-byte SHA-1 hashes — not text. Any
byte string that isn't valid UTF-8 is shown as a small sentinel object,
`{ "_bencode_bytes_hex": "…" }`, holding the bytes as hex. The encoder understands
the same sentinel, so **decode then encode reproduces the original bytes exactly**.

### Notes

Bencode has **no boolean or null** type, so JSON `true`/`false`/`null` can't be
encoded — use integers (`0`/`1`) instead.

### Privacy

Everything runs **in your browser** via WebAssembly — your data is **never
uploaded** to a server. Also available from the [gizza CLI](/) and in chat.

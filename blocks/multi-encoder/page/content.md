## Encode / decode text in your browser

One tool for the common text codecs — pick a scheme and a direction. Everything
runs locally in your browser; your text is never uploaded.

### Schemes

- **base64** — standard Base64 (RFC 4648).
- **hex** — lowercase hex of the UTF-8 bytes (decode tolerates spaces).
- **binary** — 8-bit per byte, space-separated.
- **url** — percent-encoding (every non-alphanumeric byte is escaped).
- **rot13** — the classic letter rotation (symmetric: encode = decode).
- **morse** — A–Z, 0–9 and common punctuation; letters separated by spaces and
  words by ` / `.

### Notes

- Decoding expects valid input for the chosen scheme and that the bytes form
  valid UTF-8 text; otherwise you'll get a clear error.
- ROT13 ignores the direction (it's its own inverse).

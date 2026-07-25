## About this tool

The Hex Byte Inspector reads a value you paste — as **hex**, **Base64**, or plain
**text** — and reports everything you usually reach for a calculator or a `xxd`
pipe to work out:

- its length as **bytes**, **bits** and **hex characters**;
- the same bytes shown in **hex** (grouped for readability), **Base64**, and
  **printable text**;
- and, optionally, which **common cryptographic value sizes** match that byte
  length — hash digests, AES/ChaCha keys, Ed25519/secp256k1 keys and signatures,
  and nonces/IVs.

It runs entirely in your browser: the value you paste never leaves your machine,
which matters when the "value" is a private key, a token, or a session secret.

### Worked example

Paste `Hi` with **Read input as** set to *Text* and you get:

```
Bytes:     2
Bits:      16
Hex chars: 4

Hex:    4869
Base64: SGk=
Text:   "Hi"
```

Now paste a 64-hex-character SHA-256 digest such as
`9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08` as *Hex* with
**Show matching crypto sizes** on, and the report ends with:

```
Bytes:     32
Bits:      256
Hex chars: 64
...
Matches (common 32-byte / 256-bit values):
  - SHA-256 / SHA3-256 / BLAKE2s digest
  - AES-256 or ChaCha20 key
  - Ed25519 public or private key
  - secp256k1 or NIST P-256 private key
```

That last block is the quick "what is this 32-byte blob likely to be?" answer.

### Options and limits

- **Read input as** — `hex` (the default), `base64`, or `text`. The hex reader is
  tolerant: ASCII whitespace, `:` `-` `,` `_` delimiters and per-byte `0x` / `\x`
  prefixes are stripped before decoding, so pasting `0x48 0x69` or `48:69` works.
- **Hex group size** — how many bytes go in each space-separated group of the Hex
  line; `0` prints it continuous. It's clamped to the range `0`–`64`.
- **Uppercase hex** and **Show matching crypto sizes** are simple on/off toggles.
- A hex value must decode to a whole number of bytes (an even count of hex
  digits) and Base64 must be a valid sequence, otherwise the tool explains what's
  wrong instead of guessing.

## FAQ

<details>
<summary>What counts as one byte, and how do bytes, bits and hex characters relate?</summary>

One **byte** is 8 **bits**, and it's written as exactly **2 hex characters**
(`00`–`ff`). So a 32-byte value is 256 bits and 64 hex characters — the three
numbers on the first lines of the report are just the same length in different
units.

</details>

<details>
<summary>My hex has spaces, colons or a 0x prefix — do I need to clean it first?</summary>

No. With **Read input as** set to *Hex*, the tool ignores ASCII whitespace and
the `:` `-` `,` `_` delimiters, and it strips per-byte `0x` and `\x` prefixes. So
`de ad be ef`, `de:ad:be:ef` and `0xde 0xad 0xbe 0xef` all decode to the same four
bytes. It only complains if a non-hex character remains or the digit count is odd.

</details>

<details>
<summary>How does it decide which cryptographic values "match"?</summary>

Purely by **byte length** — it never inspects or validates the content. A 32-byte
value is the size of a SHA-256 digest, an AES-256 key, and an Ed25519 key, so all
of those are listed as *possibilities*. It's a hint to narrow down what an unknown
blob might be, not a claim that your value actually is any of them.

</details>

<details>
<summary>Is my data sent anywhere?</summary>

No. The inspector is compiled to WebAssembly and runs locally in your browser, so
the value you paste — including private keys or secrets — stays on your machine.

</details>

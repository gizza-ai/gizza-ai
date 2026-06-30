## About this tool

This SHA-3 hash generator computes the **FIPS-202 SHA-3 digest** of any text
right in your browser. The hashing runs locally in WebAssembly — your input is
**never uploaded** to a server, which makes it safe for keys, tokens, and other
sensitive strings.

A hash is a fixed-length fingerprint of its input. The same text always produces
the same digest, but the digest cannot be reversed back into the original — so
hashes are ideal for verifying that data has not changed, fingerprinting
content, and deriving identifiers.

### SHA-3 vs. Keccak — they are not the same

This tool implements **FIPS-202 SHA-3**, the version NIST standardized after the
hash competition, which uses `0x06` multi-rate padding. The **original Keccak**
submission used `0x01` padding. Because the padding differs, **SHA3-256 and
Keccak-256 produce completely different digests for the same input** — they are
not interchangeable.

- Use **SHA-3** (this tool) when you need the **NIST standard** (FIPS-202).
- Use **Keccak** (in the Keccak Hash Generator) when you need the hash that
  **Ethereum** and the wider EVM ecosystem use.

### Supported variants

- **SHA3-256** (default) — a 256-bit / 32-byte digest.
- **SHA3-384** — a 384-bit / 48-byte digest.
- **SHA3-512** — a 512-bit / 64-byte digest.

### Options

- **Variant** — choose SHA3-256, SHA3-384, or SHA3-512.
- **Interpret input as** — hash the text as plain UTF-8 (default), or decode it
  from **hex** (a leading `0x` is accepted) or **base64** first so you can hash
  existing raw bytes such as a key or a file's contents.
- **Output format** — return the digest as **hex** (default) or as **base64**.
- **Uppercase hex** — emit the hex digest in uppercase.

### Notes

- A hash is a one-way function: a digest cannot be reversed back into the
  original text.
- SHA-3 is built on the Keccak sponge construction but is **not** related to the
  older SHA-1 or SHA-2 families by design — it was selected as a structurally
  different backup standard.
- For the original Keccak (Keccak-256 / Keccak-512) used by Ethereum, use the
  Keccak Hash Generator tool instead.

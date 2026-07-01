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

## FAQ

<details>
<summary>Why doesn't my SHA3-256 match a Keccak-256 tool?</summary>

Because they aren't the same algorithm. This tool implements **FIPS-202
SHA-3**, which uses `0x06` multi-rate padding; the original **Keccak** (what
Ethereum uses) uses `0x01` padding. The differing padding means the two
produce completely different digests for identical input — use the Keccak Hash
Generator when you need the Ethereum-compatible value.

</details>

<details>
<summary>Can I hash raw bytes instead of a text string?</summary>

Yes. Set **Interpret input as** to **hex** or **base64** and the tool decodes
the input to raw bytes before hashing — handy for hashing a key, a file's
contents or another digest. A leading `0x` on hex input is accepted; invalid
hex or base64 is reported as an error rather than hashed literally.

</details>

<details>
<summary>What's the difference between the three variants?</summary>

Digest size: **SHA3-256** produces 32 bytes, **SHA3-384** 48 bytes and
**SHA3-512** 64 bytes. Larger digests give a wider security margin at slightly
more output length; SHA3-256 is the default and the most common choice for
checksums and fingerprints.

</details>

<details>
<summary>Does the uppercase option change the hash itself?</summary>

No — it only affects how the same digest is displayed as **hex** (base64
output ignores it). `A1B2...` and `a1b2...` are the identical SHA-3 hash. All
hashing runs in your browser, so the text you enter is never uploaded.

</details>

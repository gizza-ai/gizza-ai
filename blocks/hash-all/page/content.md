## About this tool

This hash-all generator computes **every common digest** of the same text at
once and lays them out in one labeled table, right in your browser. The hashing
runs locally in WebAssembly — your input is **never uploaded** to a server,
which makes it safe for passwords, keys, and other sensitive strings.

A hash is a fixed-length fingerprint of its input. The same text always produces
the same digest, but the digest cannot be reversed back into the original — so
hashes are ideal for verifying that data has not changed, fingerprinting
content, and deriving identifiers. Computing all of them at once is handy when
you do not yet know which algorithm a system expects, or when you want to compare
a value against several candidates side by side.

### Algorithms computed

- **CRC-32** — a fast non-cryptographic checksum (the IEEE variant used by zip,
  gzip, and PNG). For error-detection only, never security.
- **MD5** and **SHA-1** — widely supported but **broken** for security use; treat
  them as checksums only, not for passwords or signatures.
- **SHA-2 family** — **SHA-224, SHA-256, SHA-384, SHA-512**. SHA-256 is the
  standard modern choice: it secures TLS certificates, Git object IDs, and
  download checksums.
- **SHA-3 family** — **SHA3-256** and **SHA3-512**, the Keccak-based NIST
  standard with a different internal design from SHA-2.
- **RIPEMD-160** — a 160-bit hash used in Bitcoin and PGP key fingerprints.
- **BLAKE2** (**BLAKE2b-512**, **BLAKE2s-256**) and **BLAKE3** — modern hashes
  that are typically faster than SHA-2 while remaining cryptographically strong.
- **Whirlpool** — a 512-bit hash from the ISO/IEC 10118-3 standard.

### Options

- **Interpret input as** — hash the text as plain UTF-8 (default), or decode it
  from **hex** or **base64** first so you can hash existing raw bytes such as a
  key or ciphertext.
- **Output format** — render every digest as **hex** (default) or as **base64**.
- **Uppercase hex** — emit the hex digests in uppercase.

### Notes

- A hash is a one-way function: a digest cannot be reversed back into the
  original text.
- To compute a single chosen algorithm use the hash-text tool; to hash an entire
  **file** use the file-hash tool instead.

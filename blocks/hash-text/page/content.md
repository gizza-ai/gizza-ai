## About this tool

This text hash generator computes a **cryptographic hash** of any text right in
your browser, with the algorithm of your choice. The hashing runs locally in
WebAssembly — your input is **never uploaded** to a server, which makes it safe
for passwords, keys, and other sensitive strings.

A hash is a fixed-length fingerprint of its input. The same text always produces
the same digest, but the digest cannot be reversed back into the original — so
hashes are ideal for verifying that data has not changed, fingerprinting
content, and deriving identifiers.

### Supported algorithms

- **MD5** and **SHA-1** — fast and widely supported, but **broken** for security
  use; treat them as checksums only, not for passwords or signatures.
- **SHA-2 family** — **SHA-224, SHA-256, SHA-384, SHA-512**. SHA-256 (the
  default) is the standard modern choice: it secures TLS certificates, Git
  object IDs, and download checksums.
- **SHA-3 family** — **SHA3-256** and **SHA3-512**, the Keccak-based NIST
  standard with a different internal design from SHA-2.
- **BLAKE2** (**BLAKE2b-512**, **BLAKE2s-256**) and **BLAKE3** — modern hashes
  that are typically faster than SHA-2 while remaining cryptographically strong.

### Options

- **Algorithm** — choose which hash to compute.
- **Interpret input as** — hash the text as plain UTF-8 (default), or decode it
  from **hex** or **base64** first so you can hash existing raw bytes such as a
  key or ciphertext.
- **Output format** — return the digest as **hex** (default) or as **base64**.
- **Uppercase hex** — emit the hex digest in uppercase.

### Notes

- A hash is a one-way function: a digest cannot be reversed back into the
  original text.
- To hash an entire **file** (and get MD5, SHA-1, SHA-256, SHA-512, and CRC-32
  at once), use the file-hash tool instead.

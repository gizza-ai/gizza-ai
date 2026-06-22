## About this tool

This SHA-256 hash generator computes the **SHA-256 (SHA-2)** digest of any text
right in your browser. The hashing runs locally in WebAssembly — your input is
**never uploaded** to a server, which makes it safe for passwords, keys, and
other sensitive strings.

SHA-256 produces a fixed 256-bit (32-byte) digest. It is the most widely used
cryptographic hash today: it secures TLS certificates, backs content-addressed
storage and Git object IDs, anchors blockchain transactions, and is the standard
choice for file-integrity and download checksums.

### Options

- **Interpret input as** — hash the text as plain UTF-8 (default), or decode it
  from **hex** or **base64** first so you can hash existing raw bytes such as a
  key or ciphertext.
- **Output format** — return the digest as 64-character **hex** (default) or as
  **base64** (44 characters).
- **Uppercase hex** — emit the hex digest in uppercase.

### Notes

- SHA-256 is a one-way function: a digest cannot be reversed back into the
  original text.
- The same input always produces the same digest, so SHA-256 is ideal for
  verifying that data has not changed.
- To hash an entire **file** (and also get MD5, SHA-1, SHA-512, and CRC-32), use
  the file-hash tool instead.

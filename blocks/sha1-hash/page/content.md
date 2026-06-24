## About this tool

This SHA-1 hash generator computes the **SHA-1** digest of any text right in
your browser. The hashing runs locally in WebAssembly — your input is **never
uploaded** to a server, which makes it safe for keys and other sensitive
strings.

SHA-1 produces a fixed 160-bit (20-byte) digest, shown as 40 hexadecimal
characters. It is still widely seen in **Git object IDs**, legacy file
checksums, TLS fingerprints, and older protocol interop, so a quick SHA-1 is
handy when matching or verifying those values.

### Options

- **Interpret input as** — hash the text as plain UTF-8 (default), or decode it
  from **hex** or **base64** first so you can hash existing raw bytes such as a
  key or ciphertext.
- **Output format** — return the digest as 40-character **hex** (default) or as
  **base64** (28 characters).
- **Uppercase hex** — emit the hex digest in uppercase.

### Notes

- **SHA-1 is cryptographically broken.** Practical collision attacks exist (the
  2017 SHAttered attack), so SHA-1 must **not** be used for digital signatures,
  certificates, or any new security context. Use **SHA-256** for security — try
  the sha256-hash tool.
- SHA-1 is a one-way function: a digest cannot be reversed back into the
  original text.
- The same input always produces the same digest, so SHA-1 is fine for
  non-adversarial integrity checks and matching legacy values.
- To hash an entire **file** (and also get MD5, SHA-256, SHA-512, and CRC-32),
  use the file-hash tool instead.

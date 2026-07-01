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

## FAQ

<details>
<summary>Why doesn't my digest match what sha256sum prints?</summary>

Almost always a trailing newline. `echo "abc" | sha256sum` hashes `abc\n`
(four bytes), while this tool hashes exactly the characters you typed —
`abc` — with no newline appended. Use `printf '%s' "abc" | sha256sum` to
compare like-for-like.

</details>

<details>
<summary>Can I hash raw bytes instead of text?</summary>

Yes. Set **Interpret input as** to `hex` or `base64` and the tool decodes
your input to bytes before hashing — handy for hashing a key, a ciphertext,
or another digest. The default `text` mode hashes the input as UTF-8.

</details>

<details>
<summary>Does the algorithm name have to be written exactly?</summary>

No — names are normalized, so `SHA-256`, `sha_256` and `sha256` are all the
same algorithm, and `blake2b` is accepted as shorthand for BLAKE2b-512.
Leaving the algorithm blank falls back to the default, SHA-256.

</details>

<details>
<summary>Can I get the original text back from a hash?</summary>

No. All of these are one-way functions — the digest can verify or fingerprint
data but cannot be reversed. If you need something reversible, you want
encryption (e.g. the aes-cipher tool), not a hash.

</details>

## About this tool

This **checksum verifier** confirms that a piece of data matches an expected
hash, right in your browser. The hashing runs locally in WebAssembly — your
input is **never uploaded** to a server, which makes it safe for sensitive
content such as keys, archives, or downloaded files pasted as text.

Software publishers ship a **checksum** (also called a hash or digest) next to a
download so you can prove the file you received is byte-for-byte the one they
released — that it was not truncated, corrupted in transit, or tampered with.
This tool recomputes the hash of your data and compares it, character for
character, with the value you expected.

### How it works

1. Paste your **data** and the **expected checksum**.
2. Leave the algorithm on **auto** to have it inferred from the checksum's
   length, or choose a specific one.
3. The tool reports **MATCH** or **MISMATCH**, the algorithm used, and both the
   expected and the freshly computed digests so you can eyeball any difference.

The expected checksum can be **hexadecimal** (optionally `0x`-prefixed, any
case, surrounding whitespace ignored) or **standard base64** — both forms are
accepted automatically.

### Algorithm auto-detection

Each algorithm produces a digest of a fixed width, so the length of your
expected checksum narrows down the family:

- **32 hex characters (16 bytes)** → MD5
- **40 (20 bytes)** → SHA-1
- **56 (28 bytes)** → SHA-224
- **64 (32 bytes)** → SHA-256, SHA3-256, BLAKE2s-256 or BLAKE3
- **96 (48 bytes)** → SHA-384
- **128 (64 bytes)** → SHA-512, SHA3-512 or BLAKE2b-512

When several algorithms share a width, **auto** mode tries each one and reports
whichever matched. If you already know the algorithm — common for published
download checksums — selecting it explicitly is faster and unambiguous.

### Supported algorithms

- **MD5** and **SHA-1** — fast and ubiquitous, but **cryptographically broken**;
  fine as plain integrity checksums, never for security.
- **SHA-2 family** — SHA-224, SHA-256, SHA-384, SHA-512. SHA-256 is the modern
  standard for download checksums and certificates.
- **SHA-3 family** — SHA3-256 and SHA3-512, the Keccak-based NIST standard.
- **BLAKE** — BLAKE2b-512, BLAKE2s-256 and BLAKE3, fast modern hashes.

### Tips

- If you have a hash but don't know what produced it, use the **Hash
  Identifier** tool to classify it first.
- To simply compute a hash of some text, use the **Text Hash Generator**.
- Set **Interpret input as** to `hex` or `base64` when your data is itself an
  encoded blob rather than plain text.

## FAQ

<details>
<summary>How does auto mode choose between SHA-256, SHA3-256 and BLAKE3?</summary>

It can't tell them apart from the checksum alone — all of them produce a 32-byte
digest. So auto mode simply computes *every* algorithm of that width against
your data and reports MATCH with whichever one agreed. If none match, you get a
MISMATCH with all the candidates' digests. When you already know the algorithm,
selecting it explicitly is both faster and unambiguous.

</details>

<details>
<summary>Why do I get MISMATCH when I'm sure the data is right?</summary>

The comparison is byte-exact, so the usual culprits are an invisible trailing
newline or spaces picked up when copying, Windows CRLF line endings versus LF,
or data that is actually an encoded blob — in that case set **Interpret input
as** to `hex` or `base64` so the raw bytes are hashed instead of the encoded
characters. The tool shows both digests so you can compare them directly.

</details>

<details>
<summary>Can the expected checksum be base64 instead of hex?</summary>

Yes. Both are accepted automatically: hexadecimal (any case, an optional `0x`
prefix, surrounding whitespace ignored) and standard base64 — handy for
`Content-MD5` headers or subresource-integrity-style values.

</details>

<details>
<summary>Is an MD5 or SHA-1 match still meaningful?</summary>

For *accidental* corruption, yes — a matching MD5 still proves the bytes came
through intact. But both algorithms are cryptographically broken, so a match is
no defense against deliberate tampering; for that, insist on a SHA-256 (or
stronger) checksum from the publisher.

</details>

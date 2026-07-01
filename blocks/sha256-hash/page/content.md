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

## FAQ

<details>
<summary>Why does my hash differ from what another tool produced for the "same" input?</summary>

Almost always an invisible byte difference: a trailing newline or space, Windows
`\r\n` line endings, or the other tool hashing a file rather than the pasted text.
Check the **Interpret input as** setting too — `deadbeef` hashed as *text* digests
the eight ASCII characters, while hashed as *hex* it digests the four raw bytes,
and the results are completely different. The uppercase toggle only changes the
display, never the digest.

</details>

<details>
<summary>How do I hash raw bytes, like a key or ciphertext, instead of text?</summary>

Set **Interpret input as** to `hex` or `base64`. The input is then decoded to its
raw bytes first and the SHA-256 is computed over those bytes — which is what you
want when the value is a key, a random nonce, or another hash rather than
human-readable text.

</details>

<details>
<summary>Can a SHA-256 hash be reversed to reveal my original text?</summary>

No — SHA-256 is a one-way function; there is no algorithm that recovers the input
from the 256-bit digest. The practical caveat: if the input was short or guessable
(a common password, a dictionary word), an attacker can brute-force candidates and
compare hashes. Which leads to…

</details>

<details>
<summary>Should I use SHA-256 for storing passwords?</summary>

No. SHA-256 is deliberately *fast*, which is exactly wrong for passwords — GPUs
can test billions of guesses per second. Use a slow, memory-hard password hash
instead: this site's **argon2-hash** tool generates Argon2id PHC strings designed
for that job. SHA-256 is the right choice for checksums, content addressing, and
integrity verification.

</details>

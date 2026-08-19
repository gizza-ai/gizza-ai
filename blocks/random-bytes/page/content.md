## About this tool

Use this random bytes generator when you need a specific number of cryptographically secure
**bytes** — an AES key, an HMAC or JWT signing secret, an IV or nonce, a password salt, a session
token, or a test fixture — and you want those exact bytes formatted for wherever they are going.
Pick the byte count, then choose an encoding: hex, padded Base64, URL-safe Base64 without padding,
binary, decimal, a C initializer list, or a Python bytes literal. The bytes are drawn in your
browser from the platform cryptographic random source; nothing is uploaded, and the optional
`seed_hex` field makes a run reproducible for tests and documented fixtures.

The byte count is what sets the strength. 32 bytes is 256 bits of entropy whether you print it as
64 hex characters or 44 Base64 characters — the encoding changes the length of the string, never
the amount of randomness behind it. That is the difference between this tool and a character-count
generator: asking for "32 hex characters" gives you only 16 bytes, or 128 bits. Set `count` to draw
up to 100 independent values in one run, for example to seed a key-rotation table or to fill a
staging environment's secrets.

### Worked example

For a deterministic example, set `bytes = 8`, `count = 3`, `encoding = hex`, `separator = auto`, and
`seed_hex = 00112233445566778899aabbccddeeff`. The result is three 16-character hex values, one per
line, followed by the summary line `3 values · 8 bytes (64 bits) each · hex · equivalent: openssl
rand -hex 8`. Switch `encoding` to `base64` and the same three draws come back as 12-character
padded Base64 strings instead. For real secrets, leave `seed_hex` blank so every run draws fresh
bytes from the cryptographic RNG.

### Limits and edge cases

- `bytes` accepts 1 to 4096 bytes per value, which covers everything from a 1-byte test vector to a
  4096-bit RSA modulus.
- `count` accepts 1 to 100 values, and `bytes × count` may not exceed 8192 random bytes per run, so
  a large-times-large request is rejected with an explicit message instead of producing a
  multi-megabyte string.
- `separator` only applies to the one-unit-per-byte encodings (hex, binary, decimal). Base64,
  Base64URL, the C array, and the Python literal have no per-byte boundary to split, so they ignore
  it rather than corrupting the encoding.
- `uppercase` affects the hex digits of the `hex` and `c-array` encodings only.
- `base64` uses the RFC 4648 standard alphabet with `=` padding, matching `openssl rand -base64`.
  `base64url` uses the URL-safe alphabet with padding stripped, which is the form JWT segments and
  URL-embedded tokens expect.
- `seed_hex` must be 8 to 128 hex digits. Whitespace inside a pasted seed is ignored. A seeded run
  is reproducible, not secret: anyone holding the seed can regenerate the identical bytes.

## FAQ

<details>
<summary>How many bytes should I generate for a key or secret?</summary>

For symmetric keys, match the algorithm: 16 bytes for AES-128, 32 bytes for AES-256 or
ChaCha20-Poly1305, and 32 bytes for an HMAC-SHA256 or JWT signing secret. Nonces and IVs are
algorithm-specific — 12 bytes for AES-GCM and ChaCha20-Poly1305, 16 bytes for AES-CBC. Password
salts are commonly 16 bytes. When nothing dictates a size, 32 bytes (256 bits) is a safe default
and is what this page starts with.

</details>

<details>
<summary>Is this the same as `openssl rand`?</summary>

It produces the same kind of output from an equally strong random source, and the summary line
prints the matching command (`openssl rand -hex 32`, `openssl rand -base64 32`) so you can compare
or script it later. The difference is where the randomness comes from: this page calls the
browser's Web Crypto random source, and the command-line version of the tool calls the operating
system's. Both are cryptographically secure generators, not the ordinary pseudo-random function a
language's `random()` uses.

</details>

<details>
<summary>What is the difference between Base64 and Base64URL here?</summary>

Both encode the identical bytes. `base64` uses the RFC 4648 standard alphabet, which includes `+`
and `/`, and pads the result with `=` so its length is a multiple of four. `base64url` swaps those
two characters for `-` and `_` and drops the padding, so the value is safe to place in a URL path,
a query string, a filename, or a JWT segment without further escaping. Choose `base64url` for
tokens and `base64` when a config file or library expects the classic padded form.

</details>

<details>
<summary>How is this different from a random token or password generator?</summary>

This tool is driven by a byte count: you ask for 32 bytes and get exactly 256 bits, rendered in the
encoding you pick. A token generator is driven by a character count over an alphabet, so 32
characters of hex is only 128 bits — useful when a field has a fixed width, but a different
question. A password generator additionally optimises for something a person can type and read, at
the cost of entropy per character. Use this page for machine-consumed key material.

</details>

<details>
<summary>Why would I set a seed?</summary>

Set `seed_hex` only when you need identical bytes on every run: a unit test with a fixed
expectation, a documentation example, or a shareable link that reproduces the same output for a
colleague. The values are then derived deterministically from the seed, so they are exactly as
secret as the seed itself. Leave the field blank for anything that will protect real data.

</details>

<details>
<summary>Can I get the raw binary bytes as a file?</summary>

Not directly — every encoding on this page is a text rendering of the bytes, and the result is
copyable and downloadable as text. To turn it back into raw bytes, generate `hex` output and decode
it where you need it, for example `xxd -r -p` on the command line or `bytes.fromhex()` in Python.
The `c-array` and `python-bytes` encodings exist so you can paste the bytes straight into source
code without a decoding step.

</details>

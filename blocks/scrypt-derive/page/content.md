## About this tool

scrypt (defined in RFC 7914, designed by Colin Percival) is a password-based key
derivation function that is deliberately **memory-hard**: computing a key requires
a large amount of RAM as well as CPU time, which makes large-scale brute-force
attacks on custom GPU/ASIC hardware far more expensive than with functions like
PBKDF2. This tool runs scrypt entirely in your browser using compiled Rust
(WebAssembly) — your password and salt are never sent anywhere.

### Inputs

- **Password** — the passphrase to derive a key from.
- **Salt** — a value mixed in so the same password yields different keys. For real
  key derivation, use a unique, random salt per key. The salt can be plain text
  (utf8), or raw bytes given as **hex** or **base64**.
- **N** — the CPU/memory cost parameter. It must be a power of two greater than 1
  (default 16384). Memory used is approximately 128 × N × r bytes, so raising N
  raises both time and memory.
- **r** — the block-size parameter (default 8); it scales memory use with N.
- **p** — the parallelization parameter (default 1).
- **Key length** — the number of output bytes (default 32 = 256 bits).
- **Output encoding** — **hex** (default) or **base64**.

The result is deterministic: identical inputs always produce the same derived key,
so you can reproduce a key on any device or platform that implements scrypt.

### Verify mode

Set **Mode** to `verify` and paste an existing key (hex or base64) into **Expected
key** to check whether your password and parameters reproduce it. The expected key's
byte length sets the derived length automatically, and the comparison runs locally.

### Choosing parameters

The RFC 7914 example for interactive logins is N=16384, r=8, p=1 (about 16 MiB of
memory). For higher-value secrets where a little extra latency is acceptable, raise
N (e.g. 1048576 with r=8, p=1 uses roughly 1 GiB). r and p let you tune the block
size and parallelism; most deployments keep r=8 and adjust N. This tool caps the
memory request (128 × N × r) at 1 GiB so the browser tab does not run out of memory.

### Common uses

- Reproduce a derived key that another system (OpenSSL `-scrypt`, Python
  `hashlib.scrypt`, Node's `crypto.scrypt`, Go `golang.org/x/crypto/scrypt`)
  produced, to confirm a match.
- Generate an encryption key from a passphrase for a personal project.
- Learn how the N, r and p cost parameters change the output and the time/memory cost.

### Notes on security

scrypt is memory-hard and a strong choice for password-based key derivation. For new
password-storage designs, Argon2id is the current first recommendation, with scrypt a
well-regarded alternative; PBKDF2 is acceptable where a memory-hard function cannot be
used. Whatever you choose, always use a unique random salt and the highest cost
parameters your latency and memory budget allow.

### Test vectors

This tool matches the published scrypt test vectors in RFC 7914 §12 (for example
N=16, r=1, p=1 over empty password and salt), so its output is interoperable with
standard libraries.

## FAQ

<details>
<summary>Why does the tool say my N or r values are too large?</summary>

scrypt needs roughly 128 × N × r bytes of RAM, and this tool caps that at 1 GiB so
the browser tab cannot run out of memory. N must also be a power of two greater
than 1, log2(N) must be less than r × 16, and r × p must stay below 2³⁰. If you
hit the cap, lower N or r.

</details>

<details>
<summary>How does verify mode know what key length to use?</summary>

You don't set a length in verify mode. Paste the expected key as hex or base64
(auto-detected: an even-length string of only hex digits is read as hex) and its
byte length determines how many bytes are derived before the comparison.

</details>

<details>
<summary>Will the derived key match OpenSSL, Node or Python scrypt output?</summary>

Yes — the output is deterministic and matches the RFC 7914 test vectors, so with
identical password, salt, N, r, p and length you get the same bytes as Python's
`hashlib.scrypt`, Node's `crypto.scrypt`, Go's `x/crypto/scrypt` or OpenSSL
`-scrypt`.

</details>

<details>
<summary>Is my password sent to a server?</summary>

No. The scrypt computation runs as compiled WebAssembly in your browser; the
password, salt and derived key never leave your device.

</details>

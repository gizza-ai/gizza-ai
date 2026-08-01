## About this tool

A **key derivation function (KDF)** turns a passphrase or a piece of key material
into a cryptographic key of a chosen length. This tool is one unified selector for
the four KDFs you actually reach for:

- **PBKDF2** (RFC 2898) — the widely-supported password-based KDF; pick a hash
  (SHA-1/256/384/512) and an iteration count.
- **scrypt** (RFC 7914) — memory-hard, tuned with the classic `N` / `r` / `p`
  parameters.
- **Argon2** — the modern memory-hard winner of the Password Hashing Competition.
  Here it returns **raw key material of your chosen length** (via
  `hash_password_into`), not a PHC storage hash — pick `argon2id` (default),
  `argon2i`, or `argon2d` and set memory, time cost, and parallelism.
- **HKDF** (RFC 5869) — an extract-and-expand KDF for turning existing
  **high-entropy** key material (a shared secret, a random seed) into one or more
  keys, with an optional `info` context string. HKDF is *not* meant for deriving
  keys from a low-entropy password — use PBKDF2, scrypt, or Argon2 for that.

Enter your secret and salt as UTF‑8 text, hex, or base64, choose the output length
in bytes, and read the derived key back as hex or base64. The computation runs
entirely in your browser via WebAssembly — nothing is uploaded, and the same
inputs always produce the same key, so results are reproducible against other
standards-compliant libraries.

## FAQ

<details>
<summary>Which algorithm should I use?</summary>

For deriving a key from a **password or passphrase**, prefer **Argon2id** (or
scrypt / PBKDF2 where you need broad compatibility) — they are deliberately slow
and memory-hard to resist brute force. For expanding an already **high-entropy**
value such as a Diffie-Hellman shared secret or a random seed into one or more
subkeys, use **HKDF**. PBKDF2 with a high iteration count remains a safe,
universally-available choice when the others aren't an option.

</details>

<details>
<summary>How is this different from a password hasher like Argon2 PHC?</summary>

A password hasher emits a self-describing **PHC string** (for example
`$argon2id$v=19$m=19456,t=2,p=1$…`) meant to be stored and later verified. This
tool instead returns **raw key bytes of the length you ask for** — the material
you feed into AES, HMAC, or another primitive. The Argon2 path uses
`hash_password_into` so you get chosen-length key material rather than a storage
hash.

</details>

<details>
<summary>Why does Argon2 require a salt of at least 8 bytes?</summary>

The Argon2 specification mandates a minimum salt length of 8 bytes, and the
reference implementation rejects anything shorter. Use a unique, random salt of
at least 16 bytes per key in production. PBKDF2 and scrypt also strongly benefit
from a random salt; HKDF's salt is optional (an empty salt is treated as a string
of zeros, per RFC 5869).

</details>

<details>
<summary>Are the results deterministic and standards-compliant?</summary>

Yes. Given the same secret, salt, parameters, and output length, every KDF here
produces the same bytes on every run, matching the published RFC test vectors
(RFC 6070 for PBKDF2, RFC 7914 for scrypt, RFC 5869 for HKDF, and the Argon2
reference vectors). That makes the output reproducible against other compliant
libraries such as OpenSSL, PyCryptodome, or Node's `crypto`.

</details>

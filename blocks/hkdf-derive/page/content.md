## About this tool

HKDF (HMAC-based Key Derivation Function, defined in **RFC 5869**) turns one piece of
input key material into one or more cryptographically strong, independent keys. It works
in two steps — **extract** then **expand** — and is the key-derivation primitive used in
TLS 1.3, the Signal protocol, the Noise framework, and many other modern designs. This
tool runs HKDF entirely in your browser using compiled Rust (WebAssembly), so your secret
and salt are never sent anywhere.

### How HKDF works

- **Extract** computes a fixed-length pseudorandom key: `PRK = HMAC-Hash(salt, IKM)`.
  This concentrates whatever entropy is in the input into a uniform key.
- **Expand** stretches the PRK into output key material of the length you ask for, mixing
  in the optional **info** string: `OKM = HKDF-Expand(PRK, info, L)`.

Running the full tool (`derive` mode) does both. The `extract` mode returns just the PRK,
which is useful when you want to expand it yourself later or inspect the intermediate value.

### Inputs

- **Input key material (IKM)** — the secret you are deriving from. This should already be
  high-entropy (a random key, a Diffie-Hellman shared secret, etc.). HKDF is **not** a
  password hash — for low-entropy passwords use PBKDF2, scrypt, or Argon2 first.
- **Salt** — an optional, non-secret value that strengthens extraction. If left empty,
  HKDF uses a string of zero bytes as the salt, per the RFC. A random salt is recommended
  when one is available.
- **Info / context** — an optional string that binds the output to a specific purpose, so
  the same IKM and salt can produce different keys for different uses (for example
  `app:encryption` vs `app:authentication`). Changing the info changes the output.
- **Encodings** — IKM, salt, and info can each be given as plain text (**utf8**), or as raw
  bytes in **hex** or **base64**.
- **Hash** — the underlying HMAC function: **SHA-256** (default), **SHA-384**, **SHA-512**,
  or **SHA-1** (legacy/interop only).
- **Key length** — the number of output bytes (default 32 = 256 bits). The maximum is
  255 × the hash output size (e.g. 8160 bytes for SHA-256).
- **Output encoding** — **hex** (default) or **base64**.

The result is deterministic: identical inputs always produce the same output, so you can
reproduce a key on any platform that implements HKDF (OpenSSL, Python's `hashlib` /
`cryptography`, Node's `crypto.hkdf`, Go's `golang.org/x/crypto/hkdf`, WebCrypto).

### Common uses

- Split one master secret or shared secret into several purpose-specific keys using
  different **info** labels.
- Reproduce a key another system derived, to confirm interoperability.
- Turn a Diffie-Hellman / ECDH shared secret into symmetric encryption and MAC keys.

### Notes on security

HKDF assumes the input key material already has sufficient entropy — it does not add work
factor or memory hardness. It is the right tool for deriving keys from strong secrets, and
the wrong tool for hashing user passwords (use a password-based KDF for that). Always use a
distinct **info** label per derived key when you need several keys from one secret.

### Test vectors

This tool matches the published HKDF test vectors in RFC 5869 Appendix A (SHA-256 and
SHA-1 cases), so its output is interoperable with standard libraries.

## FAQ

<details>
<summary>What's the difference between the derive and extract modes?</summary>

`derive` (the default) runs the full HKDF pipeline — extract, then expand — and
returns the number of output bytes you asked for. `extract` stops after the
first step and returns only the intermediate pseudorandom key (PRK), which is
useful for inspecting the intermediate value or expanding it yourself later.

</details>

<details>
<summary>Is it OK to leave the salt field empty?</summary>

Yes — per RFC 5869, an empty salt is replaced by a string of zero bytes the
same length as the hash output, so derivation still works and matches other
HKDF implementations. A random, non-secret salt is still recommended whenever
you can store one.

</details>

<details>
<summary>How many bytes can I derive at once?</summary>

The default is 32 bytes (256 bits), and the maximum is 255 × the hash output
size — 8,160 bytes with SHA-256. Ask for more and HKDF-Expand rejects the
request by design, not as a tool limitation.

</details>

<details>
<summary>Can I use this to hash a user's password?</summary>

No. HKDF adds no work factor or memory hardness, so it does nothing to slow
down guessing attacks on low-entropy input. Run passwords through PBKDF2,
scrypt, or Argon2 first; HKDF is for stretching secrets that are already
strong (random keys, DH/ECDH shared secrets).

</details>

## About this tool

`EVP_BytesToKey` is the legacy key-derivation function built into OpenSSL. It is
what `openssl enc -pass pass:…` uses to turn a password into the symmetric **key**
and **IV** for a cipher when you do not pass `-pbkdf2`. This tool reproduces that
exact derivation entirely in your browser using compiled Rust (WebAssembly) — your
password and salt are never sent anywhere.

### How EVP_BytesToKey works

It hashes the password (and salt) repeatedly and concatenates the digests until it
has enough bytes for the key and IV:

```
D_1 = HASH(password ‖ salt)
D_i = HASH(D_{i-1} ‖ password ‖ salt)
key ‖ iv = D_1 ‖ D_2 ‖ …   (first key_length bytes = key, next iv_length = IV)
```

With an iteration `count` greater than 1, each digest block is hashed that many
times before being appended.

### Inputs

- **Password** — the passphrase, exactly as you typed it into OpenSSL.
- **Salt** — OpenSSL uses an 8-byte salt with `-S`, prepended to encrypted files
  after the `Salted__` magic. Paste those bytes here. The salt can be plain text
  (utf8), or raw bytes given as **hex** or **base64**. Leave it empty to match
  `-nosalt`.
- **Hash** — the digest OpenSSL's `-md` selected: **MD5** (the historical default),
  **SHA-1**, **SHA-256** (the modern default), or **SHA-512**.
- **Key length** — output key bytes. **32** = AES-256, **24** = AES-192, **16** =
  AES-128, **8** = DES.
- **IV length** — output IV bytes. **16** for AES (the block size); **0** for
  stream ciphers or modes that take no IV.
- **Iteration count** — OpenSSL's `-iter` (default **1**).
- **Output encoding** — **hex** (default) or **base64**.

The result is deterministic: identical inputs always reproduce the same key and IV.

### Common uses

- **Decrypt an old OpenSSL file** — recover the exact key and IV `openssl enc`
  derived from your password so you can decrypt the ciphertext with any AES tool.
- **Interoperate with CryptoJS** — `CryptoJS.AES.encrypt(text, "passphrase")` uses
  this same MD5-based derivation with a random salt; reproduce its key/IV here.
- **Audit or learn** — see precisely how a password becomes a key and IV.

### Notes on security

`EVP_BytesToKey` is a **legacy** KDF and is weak by modern standards: with the
default count of 1 it is essentially a single hash, offering almost no resistance to
brute-force. It exists for backward compatibility. For new designs use PBKDF2
(OpenSSL's `-pbkdf2`), scrypt, or Argon2id, always with a unique random salt and a
high iteration count.

### Verified against OpenSSL

This tool matches OpenSSL's output byte-for-byte. For example,
`openssl enc -aes-256-cbc -md sha256 -nosalt -pass pass:password -P` yields the same
key and IV this tool produces for password `password`, hash `sha256`, key 32, IV 16.

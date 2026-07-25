## About this tool

NaCl Secretbox is the classic libsodium `crypto_secretbox` construction: XSalsa20 for encryption plus Poly1305 for authentication. It is designed for shared-key messages where both sides already have the same 32-byte key and can choose a unique 24-byte nonce for every message.

This tool seals plaintext into a combined `nonce || ciphertext || tag` blob, or opens a combined blob and verifies the Poly1305 tag before returning the plaintext. It is useful when you need to inspect or interoperate with Secretbox-style payloads from libsodium-compatible systems without sending data to a server.

### Worked examples

- Use **Encrypt text → base64 box** to seal `attack at dawn` with the sample 32-byte key and 24-byte nonce. The output is a base64-encoded combined box containing the nonce prefix, ciphertext, and 16-byte authentication tag.
- Use **Encrypt text → hex box** when another system expects lowercase hexadecimal bytes.
- Use **Decrypt combined base64 box** to open the sample combined box and recover the exact plaintext `attack at dawn`.

### Limits and edge cases

- The key must decode to exactly 32 bytes. A password or passphrase is not stretched into a key; derive a key first with a KDF tool if needed.
- The nonce must decode to exactly 24 bytes and must be unique for every message encrypted with the same key. Reusing a nonce with Secretbox can reveal plaintext relationships.
- Encryption requires an explicit nonce so results are reproducible across CLI/page tests. Decryption can read the nonce from the combined box, or accept a separate nonce when `data` contains only `ciphertext || tag`.
- This is NaCl Secretbox / XSalsa20-Poly1305, not IETF ChaCha20-Poly1305 and not public-key sealed boxes.

## FAQ

<details>
<summary>What should I put in the nonce field?</summary>

Use 24 random bytes encoded as hex or base64, and never reuse the same nonce with the same key. The nonce is not secret, so encryption output prepends it to the ciphertext automatically.

</details>

<details>
<summary>Can I use a password as the key?</summary>

Only if the password text is exactly 32 UTF-8 bytes and you intentionally select `key_encoding=text`. In most real workflows you should first derive a 32-byte key from a password with a dedicated KDF such as Argon2, scrypt, or PBKDF2.

</details>

<details>
<summary>Why does decryption fail after I edit one byte?</summary>

Secretbox is authenticated encryption. The Poly1305 tag covers the ciphertext, so any wrong key, wrong nonce, truncated data, or tampered byte causes tag verification to fail before plaintext is returned.

</details>

<details>
<summary>Is this compatible with libsodium crypto_secretbox?</summary>

Yes for the primitive and byte sizes: 32-byte key, 24-byte nonce, XSalsa20 stream encryption, and a 16-byte Poly1305 tag. This tool's encryption output format is a convenient combined blob: `nonce || ciphertext || tag`.

</details>

## About this tool

AES Key Wrap protects one cryptographic key with another. You supply a **key-encryption
key (KEK)** and the **key material** you want to wrap; the tool returns a wrapped blob that
is 8 bytes longer than the (padded) input and carries a built-in integrity check. Unwrapping
reverses the process and fails loudly if the KEK, algorithm, or data is wrong — so a corrupted
or tampered blob never silently decrypts to garbage.

Everything runs locally in your browser via WebAssembly. Your keys are never uploaded.

## KW vs KWP

- **KW (RFC 3394 / NIST SP 800-38F)** — the classic algorithm. The key material must be a
  non-empty multiple of 8 bytes and at least 16 bytes. Use it to wrap a 128/192/256-bit
  symmetric key.
- **KWP (RFC 5649, "key wrap with padding")** — wraps key material of *any* length from 1
  byte up, padding internally. Use it when your key isn't an 8-byte multiple.

## Key sizes

The length of the KEK selects the AES variant automatically:

- **16 bytes → AES-128**
- **24 bytes → AES-192**
- **32 bytes → AES-256**

Provide the KEK, the key material, and the wrapped output as **hex** or **base64** (your choice).

## When to use it

Key wrapping is how key-management systems store a data key under a master key (KEK): the data
key is wrapped and stored next to the data, and only an operator holding the KEK can unwrap it.
It is also the AES-KW / AES-KWP construction used inside JOSE/JWE (`A128KW`, `A256KWP`, …) and
PKCS#11.

If you want to encrypt arbitrary **plaintext** rather than another key, use the
[AES cipher](/tools/aes-cipher/) tool (CBC/CTR/GCM/ECB) or, for passphrase-based encryption with
a random salt and nonce, [text encrypt](/tools/text-encrypt/).

## Notes

- Wrapping is deterministic — wrapping the same key material under the same KEK always yields the
  same blob (there is no IV/nonce to supply).
- The integrity check is what makes unwrap safe: a wrong KEK or a flipped bit yields an error,
  not a wrong key.
- This is a low-level primitive. The KEK must itself be a strong, randomly generated key.

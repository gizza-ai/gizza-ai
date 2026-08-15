## About this tool

ECIES is hybrid public-key encryption for elliptic-curve keys. The sender generates an ephemeral key pair, performs ECDH with the recipient public key, expands the shared secret with HKDF-SHA256, and encrypts the message with an authenticated cipher. The encrypted payload carries the ephemeral public key, nonce, authentication tag, and ciphertext so the recipient can decrypt with the matching private key.

This local browser tool supports secp256k1, P-256, and P-384 keys. Keys can be pasted as SEC1 hex/base64 or PEM, and payloads can be encoded as base64 or hex. The default layout matches the common `ecies.py`/`ecies.js` style: `ephemeral public key || nonce || 16-byte tag || ciphertext`, using HKDF over the ephemeral public key plus shared point and AES-256-GCM with a 16-byte nonce.

### Worked example

Encrypt a message to a secp256k1 recipient public key with a deterministic test nonce and ephemeral private key:

```bash
gizza tool ecies-encrypt operation=encrypt data='attack at dawn' key=044f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa385b6b1b8ead809ca67454d9683fcf2ba03456d6fe2c4abe2b07f0fbdbb2f1c1 curve=secp256k1 cipher=aes-256-gcm nonce_length=16 nonce=000102030405060708090a0b0c0d0e0f ephemeral_key=2222222222222222222222222222222222222222222222222222222222222222 output_encoding=hex
```

Decrypt the resulting payload by switching `operation=decrypt`, passing the recipient private scalar as `key`, and setting `data_encoding=hex` when the payload is hex.

### Limits and edge cases

- `data` is limited to 1 MiB after decoding so the browser and wasm sandbox stay responsive.
- AES-256-GCM accepts 16-byte nonces for compatibility with common ECIES libraries and 12-byte nonces for standard GCM usage. XChaCha20-Poly1305 always uses a 24-byte nonce.
- Leave `nonce` and `ephemeral_key` blank for normal encryption so the tool generates fresh random values. Provide them only for reproducible vectors.
- Decryption verifies the authentication tag. A wrong key, curve, cipher, nonce length, KDF mode, or altered ciphertext returns an error instead of plaintext.
- This is ECIES-style hybrid encryption, not age, PGP, HPKE, NaCl sealed boxes, or password-based encryption.

## FAQ

<details>
<summary>Which key should I paste?</summary>

For encryption, paste the recipient public key. For decryption, paste the matching recipient private key. The tool accepts SEC1 public keys and private scalars as hex/base64, plus common PEM public and private key blocks.

</details>

<details>
<summary>Why does the encrypted payload start with an ephemeral public key?</summary>

ECIES generates a one-time ephemeral key for each encryption. The recipient needs the ephemeral public key to repeat ECDH during decryption, so it is stored at the front of the payload before the nonce, tag, and ciphertext.

</details>

<details>
<summary>Which KDF input should I choose?</summary>

Use `ephemeral-and-point` when you want compatibility with common ECIES libraries that bind both the ephemeral public key and shared point into HKDF. Use `shared-x` for stacks that derive from only the ECDH shared X coordinate.

</details>

<details>
<summary>Can this generate or manage long-term keys?</summary>

No. This tool encrypts and decrypts with keys you provide. Generate and store long-term private keys with a dedicated key-management tool, then paste only the public key for encryption or the private key for local decryption.

</details>

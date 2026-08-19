## About this tool

NaCl crypto_box is public-key authenticated encryption: the sender combines their Curve25519 secret key with the recipient's Curve25519 public key, then seals the message with XSalsa20-Poly1305. The recipient opens the box with their secret key and the sender's public key. If any key, nonce, ciphertext, or authentication tag byte changes, decryption fails instead of returning corrupted plaintext.

This local browser tool accepts raw 32-byte keys as hex or base64. Encryption requires a 24-byte nonce and returns one combined value: `nonce || ciphertext || 16-byte Poly1305 tag`, encoded as base64 by default. Decryption can read that combined value directly, or you can pass the nonce separately when your input is only `ciphertext || tag`.

### Worked example

Use Alice's RFC 7748 test-vector secret key to encrypt for Bob's public key:

```bash
gizza tool nacl-box-encrypt operation=encrypt data='attack at dawn' recipient_key=de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f sender_key=77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a nonce=000102030405060708090a0b0c0d0e0f1011121314151617 output_encoding=hex
```

To decrypt, swap key roles: Bob's secret key becomes `recipient_key`, Alice's public key becomes `sender_key`, and `data` is the combined box.

### Limits and edge cases

- Keys must decode to exactly 32 bytes; PEM and passphrases are intentionally not parsed here.
- Nonces must decode to exactly 24 bytes and must be unique for each sender/recipient key pair.
- The tool performs deterministic encryption for a supplied nonce; it does not generate random nonces.
- Decryption returns UTF-8 plaintext when valid. Non-UTF-8 plaintext bytes are encoded with `output_encoding`.
- This is NaCl `crypto_box` (`Curve25519 + XSalsa20-Poly1305`), not sealed boxes, age, PGP, or secretbox.

## FAQ

<details>
<summary>Which key goes in recipient_key and sender_key?</summary>

For encryption, `recipient_key` is the recipient's public key and `sender_key` is the sender's secret key. For decryption, `recipient_key` is the recipient's secret key and `sender_key` is the sender's public key. That mirrors how `crypto_box` authenticates both parties.

</details>

<details>
<summary>Why does encryption require a nonce?</summary>

NaCl crypto_box needs a unique 24-byte nonce for every message encrypted with the same sender/recipient key pair. Reusing a nonce with the same keys can reveal information about the plaintexts, so this tool makes the nonce explicit instead of silently generating one you might lose.

</details>

<details>
<summary>What is included in the encrypted output?</summary>

Encryption returns `nonce || ciphertext || tag`. The first 24 bytes are the nonce, the middle bytes are the encrypted message, and the final 16 bytes are the Poly1305 authentication tag. Decryption accepts that combined format by default.

</details>

<details>
<summary>How is this different from secretbox?</summary>

Secretbox uses one shared 32-byte symmetric key. Crypto_box uses Curve25519 public-key agreement, so the sender uses their secret key plus the recipient's public key and the recipient uses their secret key plus the sender's public key.

</details>

## About this tool

**RC6 cipher** encrypts or decrypts data with **RC6-32/20**, the standard 32-bit-word, 20-round RC6 parameterisation that was an AES finalist. It supports **ECB** and **CBC** mode plus hex or base64 key / IV / ciphertext encoding.

> ⚠️ **RC6 is not a modern default.** It is interesting for interop, learning, and CTFs, but it was not standardized as AES and is rarely deployed today. For new encryption, use **AES** (the `aes-cipher` tool) or a passphrase-based authenticated-encryption tool.

- **Key:** 1–255 bytes (encoded with the chosen format); common sizes are 16, 24, or 32 bytes.
- **IV:** 16 bytes (CBC only).
- **Padding:** PKCS#7 to the 16-byte block. **ECB** reveals patterns — prefer **CBC** when you need this cipher.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never leave the device. Also available from the [gizza CLI](/) and in chat.

## FAQ

<details>
<summary>What key sizes does RC6 accept?</summary>

Any key from 1 to 255 bytes — the RC6 key schedule expands whatever you give
it into the round keys. The sizes from the AES submission are 16, 24, or 32
bytes; anything much shorter is trivially brute-forceable, so treat 16 bytes
as the practical minimum.

</details>

<details>
<summary>Do I need an IV, and does it have to match on decrypt?</summary>

In **CBC** mode (the default), yes — a 16-byte IV encoded in your chosen
format, and decryption must use the exact IV that encrypted the data. In
**ECB** mode there is no IV at all; leave the field empty.

</details>

<details>
<summary>Why does decryption fail with a padding error?</summary>

Ciphertext is padded with PKCS#7, and a wrong key, wrong IV, wrong mode, or a
ciphertext decoded with the wrong format (hex vs base64) almost always
produces garbage that fails the padding check. Double-check that all four
settings match the ones used to encrypt.

</details>

<details>
<summary>Will other RC6 implementations read this ciphertext?</summary>

Yes, provided they use the same parameterisation: RC6-32/20 (32-bit words, 20
rounds, 128-bit block, little-endian word loading), the same mode (CBC or
ECB), and PKCS#7 padding. Those are the standard choices from the RC6 AES
submission, so most libraries interoperate.

</details>

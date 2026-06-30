## About this tool

**RC6 cipher** encrypts or decrypts data with **RC6-32/20**, the standard 32-bit-word, 20-round RC6 parameterisation that was an AES finalist. It supports **ECB** and **CBC** mode plus hex or base64 key / IV / ciphertext encoding.

> ⚠️ **RC6 is not a modern default.** It is interesting for interop, learning, and CTFs, but it was not standardized as AES and is rarely deployed today. For new encryption, use **AES** (the `aes-cipher` tool) or a passphrase-based authenticated-encryption tool.

- **Key:** 1–255 bytes (encoded with the chosen format); common sizes are 16, 24, or 32 bytes.
- **IV:** 16 bytes (CBC only).
- **Padding:** PKCS#7 to the 16-byte block. **ECB** reveals patterns — prefer **CBC** when you need this cipher.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never leave the device. Also available from the [gizza CLI](/) and in chat.

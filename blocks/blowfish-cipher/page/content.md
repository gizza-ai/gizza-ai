## About this tool

**Blowfish cipher** encrypts or decrypts data with **Blowfish** in **ECB** or
**CBC** mode, with hex or base64 key/IV/ciphertext. Blowfish takes a
**variable-length key from 4 to 56 bytes** (32–448 bits) and operates on 64-bit
(8-byte) blocks.

> ⚠️ **Blowfish is a legacy cipher.** Its 64-bit block size makes it vulnerable
> to birthday/**Sweet32** attacks when encrypting large amounts of data with one
> key. Use this tool only to **decrypt legacy data** or for **interop** with old
> systems — for new encryption use **AES** (the `aes-cipher` tool) or a
> passphrase tool.

- **Key:** 4–56 bytes. **IV:** 8 bytes (CBC only).
- **CBC** uses PKCS#7 padding; **ECB** too (and reveals patterns — avoid).

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

## About this tool

**DES cipher** encrypts or decrypts data with **single DES** in **ECB** or **CBC**
mode, with hex or base64 key/IV/ciphertext.

> ⚠️ **DES is not secure.** Its 56-bit key can be brute-forced. Use this tool only
> to **decrypt legacy data** or for **interop** with old systems — for real
> encryption use **AES** (the `aes-cipher` tool) or a passphrase tool.

- **Key:** 8 bytes. **IV:** 8 bytes (CBC only).
- **CBC** uses PKCS#7 padding; **ECB** too (and reveals patterns — avoid).

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

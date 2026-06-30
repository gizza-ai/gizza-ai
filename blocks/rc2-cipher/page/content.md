## About this tool

**RC2 cipher** encrypts or decrypts data with the **RC2 block cipher** (defined in
**RFC 2268**) in **ECB** or **CBC** mode, with hex or base64 key / IV / ciphertext.
RC2 has a 64-bit block and a separate **effective key length** (T1) that you can
configure independently of the key bytes.

> ⚠️ **RC2 is a legacy cipher and is not secure for new designs.** It survives mainly
> in old **PKCS#12** key stores, **S/MIME** messages, and some Microsoft formats. Use
> this tool only to **decrypt legacy data** or for **interop** with old systems — for
> real encryption use **AES** (the `aes-cipher` tool) or a passphrase tool.

- **Key:** 1–128 bytes (encoded with the chosen format).
- **IV:** 8 bytes (CBC only).
- **Effective key bits (T1):** 1–1024; `0` means "use the key's full bit-length."
  The same value must be set for both encrypt and decrypt.
- **Padding:** PKCS#7 to the 8-byte block. **ECB** reveals patterns — prefer **CBC**.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never leave
the device. Also available from the [gizza CLI](/) and in chat.

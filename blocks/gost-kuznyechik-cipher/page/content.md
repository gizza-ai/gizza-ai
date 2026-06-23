## About this tool

**GOST Kuznyechik cipher** is a low-level encrypt/decrypt tool for the GOST R
34.12-2015 "Kuznyechik" block cipher (also specified in **RFC 7801**), the modern
Russian standard symmetric cipher. You supply the raw **key** and **IV/nonce**,
pick the **mode**, and get the result — handy for implementing or testing against
the standard, debugging interop, or learning how the cipher works.

- **Block / key:** Kuznyechik uses a 128-bit (16-byte) block and a fixed **256-bit
  (32-byte) key**.
- **Modes:** `CBC` (default), `CTR`, `CFB`, `OFB`, and `ECB` (insecure, reveals
  patterns — included for completeness only). CBC and ECB use **PKCS7** padding;
  CTR/CFB/OFB are stream modes (no padding).
- **Encoding:** key, IV and ciphertext are **hex** or **base64**; the plaintext is
  UTF-8 text. CBC/CTR/CFB/OFB need a 16-byte IV; ECB needs none.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

### Not sure which tool you want?

If you just want to **protect a message with a passphrase** (and have the salt,
key derivation and nonce handled safely for you), use the **text-encrypt** tool
instead — `gost-kuznyechik-cipher` is for when you already have a specific raw
key, IV and mode.

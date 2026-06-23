## About this tool

**GOST Magma cipher** is a low-level encrypt/decrypt tool for the GOST 28147-89 /
GOST R 34.12-2015 **"Magma"** block cipher (also specified in **RFC 8891**), the
legacy 64-bit Russian standard symmetric cipher. You supply the raw **key** and
**IV**, pick the **mode**, and get the result — handy for implementing or testing
against the standard, debugging interop, or learning how the cipher works.

- **Block / key:** Magma uses a 64-bit (8-byte) block and a fixed **256-bit
  (32-byte) key**. The S-box is the standard `id-tc26-gost-28147-param-Z` set.
- **Modes:** `CBC` (default) and `ECB` (insecure, reveals patterns — included for
  completeness only). Both modes use **PKCS7** padding.
- **Encoding:** key, IV and ciphertext are **hex** or **base64**; the plaintext is
  UTF-8 text. CBC needs an 8-byte IV; ECB needs none.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

### Not sure which tool you want?

If you just want to **protect a message with a passphrase** (and have the salt,
key derivation and nonce handled safely for you), use the **text-encrypt** tool
instead — `gost-magma-cipher` is for when you already have a specific raw key, IV
and mode. For the newer 128-bit GOST cipher, see **gost-kuznyechik-cipher**.

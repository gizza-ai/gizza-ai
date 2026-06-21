## About this tool

**AES cipher** is a low-level AES encrypt/decrypt tool for developers: you supply
the raw **key** and **IV/nonce**, pick the **mode**, and get the result — handy for
implementing or testing against a spec, debugging interop, or learning how the
modes differ.

- **Modes:** `GCM` (authenticated — detects tampering, recommended), `CBC`, `CTR`,
  and `ECB` (insecure, reveals patterns — included for completeness only).
- **Key sizes:** AES-128/192/256, selected automatically by the key length
  (16/24/32 bytes).
- **Encoding:** key, IV and ciphertext are **hex** or **base64**; the plaintext is
  UTF-8 text. For GCM the 16-byte authentication tag is appended to the ciphertext.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

### Not sure which tool you want?

If you just want to **protect a message with a passphrase** (and have the salt,
key derivation and nonce handled safely for you), use the **text-encrypt** tool
instead — `aes-cipher` is for when you already have a specific raw key, IV and
mode.

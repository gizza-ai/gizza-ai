## About this tool

**RC4 cipher** encrypts and decrypts text with the classic **RC4** stream cipher
(also known as ARCFOUR). You supply the **key**, optionally a **drop-N**, and pick
the **encoding** — handy for interoperating with legacy systems, solving CTFs,
testing against a spec, or learning how stream ciphers work.

- **Symmetric:** RC4 XORs your data with a key-derived keystream, so the same
  operation encrypts and decrypts. To recover a message, use the *same key, drop-N
  and encoding* you encrypted it with.
- **Key:** 1–256 bytes. Enter it as a **text** passphrase, or as an **encoded**
  (hex / base64) byte string.
- **Drop-N (RC4-drop[n]):** discards the first *n* keystream bytes (common values:
  768 or 3072) to skip RC4's statistically biased prefix. Leave it at `0` for plain
  RC4. It must match on both encrypt and decrypt.
- **Encoding:** the ciphertext (and an encoded key) are **hex** or **base64**; the
  plaintext is always UTF-8 text.

### Security warning

RC4 is **cryptographically broken** — practical attacks recover plaintext, and it
is banned from TLS. **Do not use it to protect real secrets.** This tool exists for
interop, CTFs, legacy data and education only. For real encryption use the
**aes-cipher** or **text-encrypt** tools instead.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

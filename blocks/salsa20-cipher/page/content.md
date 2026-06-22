## About this tool

**Salsa20 cipher** encrypts and decrypts text with the **Salsa20** stream cipher
(the original 20-round **Salsa20/20** designed by Daniel J. Bernstein, an eSTREAM
portfolio cipher and the ancestor of ChaCha20). You supply the **key** and a
**nonce**, pick the **encoding**, and run it entirely in your browser — handy for
interop, CTFs, testing against a spec, or learning how modern stream ciphers work.

- **Symmetric:** Salsa20 expands the key + nonce into a keystream that is XOR'd
  with your data, so the same operation encrypts and decrypts. To recover a
  message, use the *same key, nonce, counter and encoding* you encrypted it with.
- **Key:** exactly **16 or 32 bytes** (128- or 256-bit). Enter it as a **text**
  string of that length, or as an **encoded** (hex / base64) byte string.
- **Nonce:** exactly **8 bytes**. Treat it like an IV — it does not need to be
  secret, but a key + nonce pair must **never be reused** for two different
  messages, or the keystream repeats and the cipher is broken.
- **Block counter:** the initial 64-bit counter (each Salsa20 block is 64 bytes).
  Leave it at `0` for normal use; set it to decrypt starting from an offset. It
  must match on both sides.
- **Encoding:** the ciphertext (and an encoded key/nonce) are **hex** or
  **base64**; the plaintext is always UTF-8 text.

### Note on security

Salsa20/20 is a well-regarded modern cipher with no practical break. But this tool
provides only the **raw cipher** — it has no authentication (no MAC), no key
derivation from a password, and reusing a nonce is catastrophic. For protecting
real files behind a password use the authenticated **aes-cipher** or
**text-encrypt** tools instead, which handle key derivation and integrity for you.

### Privacy

Everything runs **in your browser** via WebAssembly — your key, nonce and data
never leave the device. Also available from the [gizza CLI](/) and in chat.

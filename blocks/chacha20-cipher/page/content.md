## About this tool

**ChaCha20 cipher** encrypts and decrypts text with the **ChaCha20** stream
cipher and the **ChaCha20-Poly1305** authenticated construction, both as
specified in **RFC 8439** (the IETF variant designed by Daniel J. Bernstein and
standardised for TLS). You supply the **key** and a **nonce**, pick a **mode**
and **encoding**, and run it entirely in your browser — handy for interop, CTFs,
testing against a spec, or learning how modern stream ciphers work.

- **Two modes:**
  - **stream** — raw ChaCha20: the key + nonce expand into a keystream that is
    XOR'd with your data. The same operation encrypts and decrypts, but there is
    **no authentication** — a flipped bit goes undetected.
  - **aead** — **ChaCha20-Poly1305**: authenticated encryption. A 16-byte
    **Poly1305** tag is appended to the ciphertext, and any **associated data
    (AAD)** you provide is authenticated too. Decryption **verifies the tag** and
    fails if the key, nonce, AAD or ciphertext don't match — so tampering is
    detected.
- **Key:** exactly **32 bytes** (256-bit). Enter it as a **text** string of that
  length, or as an **encoded** (hex / base64) byte string.
- **Nonce:** exactly **12 bytes** (96-bit, the IETF/RFC 8439 nonce size). Treat
  it like an IV — it does not need to be secret, but a key + nonce pair must
  **never be reused** for two different messages, or the keystream repeats and
  the cipher is broken.
- **Associated data (AAD):** optional, **aead** mode only — extra data (e.g. a
  header or version tag) that is authenticated but **not** encrypted. It must
  match on both sides.
- **Block counter:** the initial 32-bit counter for **stream** mode (each block
  is 64 bytes). Leave it at `0` for normal use. AEAD ignores it (RFC 8439 fixes
  the counter).
- **Encoding:** the ciphertext (and an encoded key/nonce) are **hex** or
  **base64**; the plaintext is always UTF-8 text. In AEAD mode the encoded value
  is the ciphertext followed by the 16-byte tag.

### Note on security

ChaCha20-Poly1305 is a modern, widely deployed AEAD with no practical break, but
this tool provides only the **raw cipher** — it has no password-based key
derivation, and reusing a nonce is catastrophic. For protecting real files behind
a password use the authenticated **aes-cipher** or **text-encrypt** tools instead,
which handle key derivation for you.

### Privacy

Everything runs **in your browser** via WebAssembly — your key, nonce and data
never leave the device. Also available from the [gizza CLI](/) and in chat.

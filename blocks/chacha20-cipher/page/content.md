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

## FAQ

<details>
<summary>Why do I get a key or nonce length error?</summary>

ChaCha20 requires **exactly 32 key bytes and 12 nonce bytes** — no padding, no
truncation. With the key format set to **text**, the length is counted in UTF-8
bytes, so a 32-character ASCII string works but accented or multi-byte characters
throw the count off. Switch the format to **encoded** and supply the key/nonce as
64 hex characters (or the base64 equivalent) to be exact.

</details>

<details>
<summary>Why does AEAD decryption fail even though my key looks right?</summary>

ChaCha20-Poly1305 verifies a 16-byte Poly1305 tag before returning anything. If the
key, nonce, AAD, or ciphertext differ by even one bit, verification fails on
purpose. Also make sure the encoded input includes the tag: this tool (like RFC
8439) appends the 16-byte tag to the ciphertext, so the value you paste must be
ciphertext + tag, not the ciphertext alone.

</details>

<details>
<summary>Can I decrypt data produced by OpenSSL or another library?</summary>

Yes, as long as it used the **IETF RFC 8439 variant** — 32-byte key, 12-byte nonce,
32-bit counter — which is what TLS and most modern libraries implement. Data from
the original Bernstein variant (8-byte nonce) or XChaCha20 (24-byte nonce) will not
match because the nonce size differs.

</details>

<details>
<summary>What does the block counter setting do?</summary>

In **stream** mode it sets the initial 32-bit block counter — each block covers 64
bytes of keystream — which lets you match implementations that start counting at 1
or resume mid-stream. Leave it at `0` for normal use. **aead** mode ignores it,
because RFC 8439 fixes the counter layout for ChaCha20-Poly1305.

</details>

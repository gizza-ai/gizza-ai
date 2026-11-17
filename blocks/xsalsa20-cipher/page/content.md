## About this tool

**XSalsa20 cipher** encrypts or decrypts UTF-8 data with XSalsa20, an extended-nonce variant of the Salsa20 stream cipher. XSalsa20 uses a 32-byte key and a 24-byte nonce, then XORs the data with a generated keystream.

- **Symmetric stream cipher:** decrypt by running the ciphertext with the same key, nonce, counter, and encoding.
- **24-byte nonce:** longer than classic Salsa20's 8-byte nonce, reducing accidental collision risk when nonces are generated randomly.
- **Encodings:** ciphertext, encoded keys, and encoded nonces can be hex or Base64.

### Security notes

Never reuse a key+nonce pair for two different messages. Reuse leaks the XOR of plaintexts. XSalsa20 provides confidentiality only; it does **not** authenticate ciphertext. Use an AEAD construction when tamper detection matters.

### Privacy

The browser version runs locally in WebAssembly. Your key, nonce, and data stay on your device.

### FAQ

<details>
<summary>Why does it complain about my key or nonce length?</summary>

XSalsa20 is strict: the key must be exactly **32 bytes** and the nonce exactly **24 bytes**. With key format `encoded` (the default) that's the decoded byte count, so 64 hex characters for the key and 48 for the nonce. With `text` it's the raw UTF-8 bytes of what you type. There's no padding or key stretching.

</details>

<details>
<summary>Is the output compatible with libsodium / NaCl?</summary>

Yes for the raw stream cipher: the keystream matches libsodium's `crypto_stream_xsalsa20_xor_ic` for the same key, nonce and block counter. The one difference is at the very end of the 2^64-block keystream: libsodium wraps back to block 0, which reuses keystream, while this tool returns an error instead. It is **not** a `crypto_secretbox` ciphertext. Secretbox adds a Poly1305 tag and starts encrypting 32 bytes into the keystream, so the two don't interoperate directly.

</details>

<details>
<summary>What is the block counter for?</summary>

It's the 64-bit starting block index of the keystream (one block = 64 bytes). Leave it at `0` normally. Set it to N to encrypt or decrypt as if you were starting 64×N bytes into the stream, for example to seek into a long message. It must be the same on both sides, and it must be a whole number ≥ 0.

</details>

<details>
<summary>Why must I never reuse a key + nonce pair?</summary>

XSalsa20 XORs your data with a keystream derived only from the key, nonce and counter. Two messages under the same pair share that keystream, so XORing the two ciphertexts cancels it and leaks both plaintexts. The 24-byte nonce is long enough to pick at random for every message, so do that.

</details>

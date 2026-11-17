## About this tool

**XSalsa20 cipher** encrypts or decrypts UTF-8 data with XSalsa20, an extended-nonce variant of the Salsa20 stream cipher. XSalsa20 uses a 32-byte key and a 24-byte nonce, then XORs the data with a generated keystream.

- **Symmetric stream cipher:** decrypt by running the ciphertext with the same key, nonce, counter, and encoding.
- **24-byte nonce:** longer than classic Salsa20's 8-byte nonce, reducing accidental collision risk when nonces are generated randomly.
- **Encodings:** ciphertext, encoded keys, and encoded nonces can be hex or Base64.

### Security notes

Never reuse a key+nonce pair for two different messages. Reuse leaks the XOR of plaintexts. XSalsa20 provides confidentiality only; it does **not** authenticate ciphertext. Use an AEAD construction when tamper detection matters.

### Privacy

The browser version runs locally in WebAssembly. Your key, nonce, and data stay on your device.

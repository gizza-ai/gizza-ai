## About this tool

**Rabbit cipher** encrypts and decrypts text with the **Rabbit** stream cipher
(RFC 4503), one of the four software-portfolio finalists of the eSTREAM project.
You supply a 128-bit **key**, an optional 64-bit **IV**, and pick the **encoding** —
handy for interoperating with other Rabbit implementations, solving CTFs, testing
against the spec, or learning how modern stream ciphers work.

- **Symmetric:** Rabbit XORs your data with a key-derived keystream, so the same
  operation encrypts and decrypts. To recover a message, use the *same key, IV and
  encoding* you encrypted it with.
- **Key:** exactly **16 bytes** (128 bits). Enter it as a **text** passphrase (16
  characters) or as an **encoded** byte string (32 hex chars or 24 base64 chars).
- **IV (optional):** exactly **8 bytes** (64 bits) when given — 8 characters of text,
  or 16 hex / 12 base64 chars. Leave it empty for no IV. A different IV produces a
  different keystream from the same key, so reuse a key safely by varying the IV. It
  must match on both encrypt and decrypt.
- **Byte order:** keys and IVs are read **most-significant-byte first**, matching the
  RFC 4503 test vectors, so hex values interoperate directly with the spec.
- **Encoding:** the ciphertext (and an encoded key/IV) are **hex** or **base64**; the
  plaintext is always UTF-8 text.

### About Rabbit

Rabbit is a fast, high-throughput stream cipher designed by Cryptico and published as
**RFC 4503**. Its internal state combines eight 32-bit state variables and eight 32-bit
counters driven by a counter-carry system; each iteration produces a 128-bit keystream
block that is XORed with the data. This tool validates against the official RFC 4503
test vectors. As with any stream cipher, **never reuse the same key + IV pair** for two
different messages.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never leave
the device. Also available from the [gizza CLI](/) and in chat.

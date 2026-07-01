## About this tool

**XOR cipher** applies a repeating bytewise XOR: every byte of your data is
XOR-ed with the next byte of the key, and the key repeats until the whole input
is transformed. Choose whether the data and key are read as text, hex, or
Base64, then choose hex, Base64, or UTF-8 for the result.

- **Symmetric:** XOR is its own inverse. To decrypt, run the ciphertext through
  the same operation with the same key and matching input format.
- **Flexible encodings:** use UTF-8 text for quick notes, hex for byte-oriented
  CTF exercises, or Base64 for compact copy/paste.
- **Repeating-key behavior:** a one-byte key applies the same mask to every
  byte; longer keys cycle across the data.

### Examples

- `Hello` with text key `K` outputs `032e272724` as hex. Feed that hex back in
  with key `K` and UTF-8 output to recover `Hello`.
- The CryptoPals Set 1 Challenge 5 vector is a repeating-key XOR with key `ICE`.
- For binary-looking output, prefer hex or Base64. UTF-8 output is for plaintext
  recovery and reports an error when the bytes are not valid UTF-8.

### Security warning

Repeating-key XOR is **not secure encryption**. It is useful for interop,
obfuscation, learning, and CTFs, but it is vulnerable to frequency analysis and
known-plaintext attacks. For real secrets, use authenticated encryption tools
such as **aes-cipher** or **text-encrypt** instead.

### Privacy

The browser version runs locally in WebAssembly: your data and key stay on your
device.

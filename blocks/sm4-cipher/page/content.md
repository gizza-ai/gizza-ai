## About this tool

**SM4 cipher** encrypts or decrypts data with **SM4** in **ECB** or **CBC**
mode, with hex or base64 key/IV/ciphertext. SM4 is the **Chinese national
standard** block cipher (**GB/T 32907-2016**, also standardised as
ISO/IEC 18033-3). It uses a **128-bit (16-byte) key** and operates on 128-bit
(16-byte) blocks.

- **Key:** exactly 16 bytes. **IV:** 16 bytes (CBC only).
- **CBC** uses PKCS#7 padding; **ECB** too. **ECB reveals patterns** in repeated
  blocks — prefer **CBC** with a random IV for anything other than interop tests.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

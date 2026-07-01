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

## FAQ

<details>
<summary>What size and format must the key and IV be?</summary>

The key must decode to **exactly 16 bytes** (128 bits) and the CBC IV to 16
bytes as well — so in hex that's 32 hex characters, and in base64 a 24-character
string ending in `==`. The **format** option sets the encoding for the key, IV,
*and* ciphertext together; the plaintext side is always plain UTF-8 text.

</details>

<details>
<summary>Why do I get "decryption failed (wrong key/iv or corrupt data)"?</summary>

Both modes use PKCS#7 padding, and that padding is verified on decrypt — a
wrong key, wrong IV, wrong mode, or a ciphertext pasted in the wrong encoding
(hex vs base64) almost always produces invalid padding and this error. If the
padding does check out but the recovered bytes aren't valid UTF-8, you'll get
a separate "not valid UTF-8 text" error instead.

</details>

<details>
<summary>When is ECB acceptable, and does it need an IV?</summary>

ECB takes no IV, which makes it convenient for interop tests and verifying
known-answer vectors — but identical plaintext blocks encrypt to identical
ciphertext blocks, leaking structure. For anything real, use **CBC** (the
default) with a fresh random 16-byte IV per message.

</details>

<details>
<summary>Is the output compatible with other SM4 implementations?</summary>

Yes. This is standard SM4 per GB/T 32907-2016 (the implementation reproduces
the specification's known-answer test vector) with PKCS#7 padding, so
ciphertext exchanges cleanly with OpenSSL's `-sm4-cbc`/`-sm4-ecb` and other
GM/T-compliant libraries as long as the key, IV, mode, and encoding match.

</details>

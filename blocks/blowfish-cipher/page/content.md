## About this tool

**Blowfish cipher** encrypts or decrypts data with **Blowfish** in **ECB** or
**CBC** mode, with hex or base64 key/IV/ciphertext. Blowfish takes a
**variable-length key from 4 to 56 bytes** (32–448 bits) and operates on 64-bit
(8-byte) blocks.

> ⚠️ **Blowfish is a legacy cipher.** Its 64-bit block size makes it vulnerable
> to birthday/**Sweet32** attacks when encrypting large amounts of data with one
> key. Use this tool only to **decrypt legacy data** or for **interop** with old
> systems — for new encryption use **AES** (the `aes-cipher` tool) or a
> passphrase tool.

- **Key:** 4–56 bytes. **IV:** 8 bytes (CBC only).
- **CBC** uses PKCS#7 padding; **ECB** too (and reveals patterns — avoid).

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

## FAQ

<details>
<summary>What key lengths does Blowfish accept?</summary>

Anything from **4 to 56 bytes** (32–448 bits). The key must be supplied in the
selected data format — base64 by default, or hex — and the tool rejects keys
outside that range with an explicit length error. Note that the key length is
measured **after** decoding, so a 16-character hex string is an 8-byte key.

</details>

<details>
<summary>Do I need an IV, and how long must it be?</summary>

Only in **CBC** mode, where the IV must be exactly **8 bytes** (Blowfish's
block size) encoded in the same format as the key. In **ECB** mode there is no
IV — leave the field empty. If CBC decryption produces garbage, the usual
culprit is a wrong or reused IV.

</details>

<details>
<summary>Why does decryption fail with a padding error?</summary>

Both modes use **PKCS#7 padding**, so the last block must unpad cleanly. A
padding failure almost always means the key, mode (CBC vs ECB), or data format
(base64 vs hex) doesn't match what was used to encrypt — not that the
ciphertext is "corrupt".

</details>

<details>
<summary>Is Blowfish still safe to use for new data?</summary>

No — its 64-bit block size makes it vulnerable to Sweet32-style birthday
attacks once you encrypt enough data under one key. Use it to decrypt legacy
data or interoperate with old systems, and pick AES (see the aes-cipher tool)
for anything new.

</details>

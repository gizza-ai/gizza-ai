## About this tool

**DES cipher** encrypts or decrypts data with **single DES** in **ECB** or **CBC**
mode, with hex or base64 key/IV/ciphertext.

> ⚠️ **DES is not secure.** Its 56-bit key can be brute-forced. Use this tool only
> to **decrypt legacy data** or for **interop** with old systems — for real
> encryption use **AES** (the `aes-cipher` tool) or a passphrase tool.

- **Key:** 8 bytes. **IV:** 8 bytes (CBC only).
- **CBC** uses PKCS#7 padding; **ECB** too (and reveals patterns — avoid).

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

## FAQ

<details>
<summary>Does this tool support Triple DES (3DES)?</summary>

No — it implements **single DES** only, so the key must be exactly **8 bytes**.
A 16- or 24-byte key (the 2-key/3-key 3DES sizes) is rejected with a length error.
If your legacy data is 3DES-encrypted, this tool can't decrypt it.

</details>

<details>
<summary>Why does decryption fail with "wrong key/iv or corrupt data"?</summary>

Both modes use **PKCS#7 padding**, and that padding is checked after decryption. A
wrong key, a wrong IV (CBC), a truncated ciphertext, or ciphertext pasted in the
wrong encoding all produce garbage padding and trigger this error. Double-check
that the **format** setting (base64 is the default, hex the alternative) matches
how your ciphertext, key, and IV are actually encoded.

</details>

<details>
<summary>Which inputs does the format setting apply to?</summary>

The hex/base64 choice applies to the **key, the IV, and the ciphertext** together —
they must all use the same encoding. The plaintext side is always plain UTF-8 text:
you type text to encrypt, and decryption returns text.

</details>

<details>
<summary>When do I need an IV?</summary>

Only in **CBC** mode (the default), where an 8-byte IV is required. **ECB** mode
takes no IV — but ECB encrypts every identical block identically, which leaks
patterns; use it only when a legacy system forces you to. And remember DES itself
is brute-forceable — for new encryption use AES instead.

</details>

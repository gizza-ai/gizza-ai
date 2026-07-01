## About this tool

**RSA encrypt** encrypts a short message to a recipient's **RSA public key** and
returns the result as **base64 ciphertext**. Only the holder of the matching
**private key** can decrypt it — so you can share the ciphertext over any channel.

- **Padding:** `oaep` (RSAES-OAEP — randomized, recommended for new systems) or
  `pkcs1v15` (RSAES-PKCS#1 v1.5 — the legacy scheme).
- **OAEP hash:** SHA-256 (default), SHA-384, or SHA-512 (used for the MGF1 and
  label digest; ignored when padding is `pkcs1v15`).
- **Key formats:** PEM, either SPKI (`-----BEGIN PUBLIC KEY-----`) or PKCS#1
  (`-----BEGIN RSA PUBLIC KEY-----`).

### Privacy

Everything runs **in your browser** via WebAssembly — the public key and the
message are never uploaded to a server. You can also run it from the
[gizza CLI](/) or inside a gizza chat.

### Notes

- RSA can only encrypt a payload that fits in **one block**: roughly **190 bytes**
  for a 2048-bit key with OAEP-SHA256, or **245 bytes** with PKCS#1 v1.5. For
  larger data, the standard approach is hybrid encryption — encrypt a random
  symmetric (AES) key with RSA and the data with that key.
- Encryption is **randomized**, so the ciphertext differs on every run even for
  the same input — that's expected and secure.
- Decrypt with any standard RSA library using the **same padding and hash** and
  the matching private key. Need a key pair? See the RSA key-pair generator tool.

## FAQ

<details>
<summary>Why do I get "message too long for this key/padding"?</summary>

RSA encrypts only a single block. With a 2048-bit key that's about **190
bytes** using OAEP-SHA256 or **245 bytes** with PKCS#1 v1.5 — larger OAEP
hashes leave even less room. For anything bigger, use hybrid encryption:
encrypt a random AES key with RSA and encrypt the actual data with that AES
key.

</details>

<details>
<summary>Which key formats and paddings are supported?</summary>

Public keys in PEM — either SPKI (`-----BEGIN PUBLIC KEY-----`) or PKCS#1
(`-----BEGIN RSA PUBLIC KEY-----`); both are auto-detected. Padding is **OAEP**
(recommended, with a SHA-256/384/512 hash for MGF1) or legacy **PKCS#1 v1.5**.
The hash choice is ignored when you pick PKCS#1 v1.5.

</details>

<details>
<summary>Why is the ciphertext different every time I encrypt the same message?</summary>

Both OAEP and PKCS#1 v1.5 are **randomized** — they mix in fresh random bytes
on each run — so identical plaintext produces different base64 output every
time. That's expected and is what keeps RSA encryption secure; any correct
private key still decrypts it back to the same message.

</details>

<details>
<summary>Is the public key or message sent to a server?</summary>

No. Encryption runs entirely in your browser via WebAssembly, so the public
key and plaintext never leave your device. To decrypt, use any standard RSA
library with the matching private key and the same padding and hash.

</details>

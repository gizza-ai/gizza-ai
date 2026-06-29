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

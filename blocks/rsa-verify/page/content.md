## About this tool

**RSA verify** checks whether an RSA **signature** is authentic for a given
**message** and **public key**. If verification succeeds, the message came from
the holder of the matching private key and was not altered in transit.

- **Schemes:** `pkcs1v15` (RSASSA-PKCS#1 v1.5 — the classic scheme) or `pss`
  (RSASSA-PSS — recommended for new systems). The scheme must match how the
  signature was produced.
- **Hashes:** SHA-256 (default), SHA-384, or SHA-512 — also must match the signer.
- **Key formats:** PEM, either SPKI (`-----BEGIN PUBLIC KEY-----`) or PKCS#1
  (`-----BEGIN RSA PUBLIC KEY-----`).
- **Signature:** base64-encoded (the raw signature bytes, as produced by the RSA
  sign tool or any standard RSA library).

### Privacy

Everything runs **in your browser** via WebAssembly — the message, signature, and
public key are never uploaded to a server. You can also run it from the
[gizza CLI](/) or inside a gizza chat.

### Notes

- A result of **VALID** means the signature matches; **INVALID** means it does not
  (wrong key, tampered message, or a mismatched scheme/hash).
- The scheme and hash must be the **same** ones used when signing. If you are not
  sure, try the common defaults (PKCS#1 v1.5 + SHA-256) first.
- Need to create a signature instead? See the RSA sign tool. Need a key pair? See
  the RSA key-pair generator tool.

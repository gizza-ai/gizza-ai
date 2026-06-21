## About this tool

**RSA sign** creates a cryptographic signature over a message using your **RSA
private key**, and returns it as **base64**. Anyone with your matching public key
can then verify that the message came from you and wasn't altered.

- **Schemes:** `pkcs1v15` (RSASSA-PKCS#1 v1.5 — deterministic, the classic scheme)
  or `pss` (RSASSA-PSS — randomized, recommended for new systems).
- **Hashes:** SHA-256 (default), SHA-384, or SHA-512.
- **Key formats:** PEM, either PKCS#8 (`-----BEGIN PRIVATE KEY-----`) or PKCS#1
  (`-----BEGIN RSA PRIVATE KEY-----`).

### Privacy

Everything runs **in your browser** via WebAssembly — your private key and the
message are never uploaded to a server. You can also run it from the
[gizza CLI](/) or inside a gizza chat.

### Notes

- This signs the **message** (it is hashed with the chosen algorithm first). The
  output is the raw signature, base64-encoded.
- Verify with any standard RSA library using the **same scheme and hash** and your
  public key. Need a key pair? See the RSA key-pair generator tool.

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

## FAQ

<details>
<summary>Why do I get a different signature every time with PSS?</summary>

That's PSS working as designed: RSASSA-PSS mixes in a fresh random salt on
every signing, so repeated runs over the same message produce different
signatures — and every one of them verifies. If you need a reproducible
signature (e.g. for a test fixture), use `pkcs1v15`, which is deterministic.

</details>

<details>
<summary>My key says "BEGIN ENCRYPTED PRIVATE KEY" — why won't it load?</summary>

Passphrase-protected keys aren't supported; the tool parses plain PKCS#8 or
PKCS#1 PEM only. Decrypt a copy first, e.g.
`openssl pkcs8 -in enc.pem -out plain.pem` (or `openssl rsa -in key.pem -out
plain.pem` for traditional keys), then paste the decrypted PEM.

</details>

<details>
<summary>How do I verify the signature with OpenSSL?</summary>

Base64-decode the output to a binary file, then run
`openssl dgst -sha256 -verify public.pem -signature sig.bin message.txt`,
matching the hash you chose here. For PSS add
`-sigopt rsa_padding_mode:pss`. The scheme and hash must match on both sides
or verification fails.

</details>

<details>
<summary>How long will the signature be?</summary>

Exactly the key's modulus size, regardless of message length: a 2048-bit key
yields a 256-byte signature (344 base64 characters), a 4096-bit key 512 bytes.
The message itself is hashed first, so signing a long text costs no more than
a short one.

</details>

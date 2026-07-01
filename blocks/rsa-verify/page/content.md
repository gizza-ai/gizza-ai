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

## FAQ

<details>
<summary>The signature is definitely right — why do I get INVALID?</summary>

Nine times out of ten the **scheme or hash doesn't match the signer**. A PSS
signature will never verify under pkcs1v15 (and vice versa), and SHA-256 vs
SHA-512 are just as incompatible. Check what the signing side used; if unsure,
start with PKCS#1 v1.5 + SHA-256, then try PSS. A wrong public key or an
altered message (even a trailing newline) also yields INVALID.

</details>

<details>
<summary>What format must the signature be in?</summary>

Standard **base64** of the raw signature bytes — exactly what OpenSSL's
`base64` output or any RSA library gives you. Hex signatures need converting to
base64 first; malformed base64 is reported as an input error, distinct from a
clean INVALID verdict.

</details>

<details>
<summary>Which public-key formats are accepted?</summary>

PEM in either form: SPKI (`-----BEGIN PUBLIC KEY-----`) or PKCS#1
(`-----BEGIN RSA PUBLIC KEY-----`). Both are tried automatically, so you don't
need to know which one you have. Private keys are not accepted — verification
only ever needs the public half.

</details>

<details>
<summary>Is an INVALID result the same thing as an error?</summary>

No. **INVALID** is a real cryptographic verdict: the inputs parsed fine but the
signature does not match the message under that key/scheme/hash. An **error**
means an input was malformed (bad PEM, bad base64, unknown scheme or hash) and
verification never ran.

</details>

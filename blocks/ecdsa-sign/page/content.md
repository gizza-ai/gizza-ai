## About this tool

**ECDSA Sign** signs a message with an elliptic-curve private key and returns the
signature in base64 and hex. Paste your PEM key, pick the curve and output
format, and sign — entirely in your browser via WebAssembly. Your key and
message are never uploaded.

### Options

- **Curve** — **P-256** (NIST secp256r1, hashed with SHA-256) or **P-384** (NIST
  secp384r1, SHA-384). The curve must match the key you paste.
- **Signature format** —
  - **DER**: ASN.1 `SEQUENCE { r, s }`, the encoding OpenSSL produces and most
    X.509 / TLS tooling expects.
  - **raw**: fixed-length `r || s` (IEEE-P1363), the form used by JOSE/JWT
    (`ES256`, `ES384`) and WebCrypto.

### Deterministic signatures

Signing uses **RFC-6979** deterministic nonces, so signing the same message with
the same key always yields the same signature — no randomness required, and no
risk of a repeated-nonce key leak.

### Key format

The key must be **PEM-encoded PKCS#8** (`-----BEGIN PRIVATE KEY-----`). To
generate one:

```
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256
```

If you have an old-style SEC1 key (`-----BEGIN EC PRIVATE KEY-----`), convert it:

```
openssl pkcs8 -topk8 -nocrypt -in sec1.pem
```

Verify a signature with the matching public key using your standard ECDSA
tooling (OpenSSL, WebCrypto, JOSE libraries).

### FAQ

<details>
<summary>Why do I get the same signature every time I sign?</summary>

That's by design: the tool uses RFC-6979 deterministic nonces, so a given key + message always produces the same signature. It still verifies exactly like a randomized ECDSA signature — determinism just removes the repeated-nonce risk.

</details>

<details>
<summary>Which format do I need for a JWT (ES256/ES384)?</summary>

Choose **raw**. JOSE/JWT and WebCrypto expect the fixed-length `r || s` encoding — 64 bytes for P-256 (ES256), 96 bytes for P-384 (ES384). **DER** is the ASN.1 form OpenSSL and X.509/TLS tooling use; the two are not interchangeable.

</details>

<details>
<summary>It says "invalid EC private key" — what's wrong?</summary>

The key must be PEM PKCS#8 (`-----BEGIN PRIVATE KEY-----`) and its curve must match the one you selected — a P-384 key won't parse when the curve is set to P-256. If your file says `BEGIN EC PRIVATE KEY` (SEC1), convert it first with `openssl pkcs8 -topk8 -nocrypt`.

</details>

<details>
<summary>Is P-521 or Ed25519 supported?</summary>

No — this tool signs with NIST **P-256** and **P-384** only (the curve fixes the hash: SHA-256 or SHA-384). Ed25519 is a different signature scheme (EdDSA), not ECDSA.

</details>

<details>
<summary>Does my private key leave the browser?</summary>

No. Signing runs locally in WebAssembly; the key and message are never sent to a server. Still, treat any pasted production key with care and prefer test keys where possible.

</details>

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

## About this tool

**JWK Thumbprint** computes the **RFC 7638** SHA-256 thumbprint of a JSON Web Key
— the canonical, deterministic identifier for a key, commonly used as its `kid`.

How it works (per the spec): only the **required members** for the key type are
kept, sorted lexicographically, serialized as compact JSON with no whitespace,
then SHA-256 hashed and base64url-encoded (no padding):

- **RSA** → `e`, `kty`, `n`
- **EC** → `crv`, `kty`, `x`, `y`
- **OKP** (Ed25519/X25519) → `crv`, `kty`, `x`
- **oct** → `k`, `kty`

The tool returns the thumbprint, the key type, and the exact canonical JSON it
hashed (so you can see what went in). Everything runs **locally in your browser**
via WebAssembly — your key is never uploaded.

### Handy for

- Deriving a stable `kid` for a key in a JWKS.
- Comparing two JWKs for equality regardless of member order or extra fields.
- Verifying a published thumbprint.

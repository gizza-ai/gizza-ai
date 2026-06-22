## About this tool

**PEM → JWK** converts a PEM-encoded cryptographic key into the equivalent
**JSON Web Key (JWK)** — the JSON key format used by JWT/JOSE libraries, OIDC
discovery (`jwks_uri`), and web crypto.

Paste a key and you get back a JWK object:

- **RSA** — accepts PKCS#1, PKCS#8, or SPKI PEM. Public keys produce
  `{ "kty": "RSA", "n", "e" }`; private keys add `d`, `p`, `q`, `dp`, `dq`, `qi`.
- **EC** — accepts SEC1, PKCS#8, or SPKI PEM over the NIST curves **P-256**,
  **P-384** and **P-521**. Public keys produce `{ "kty": "EC", "crv", "x", "y" }`;
  private keys add `d`.

All binary members are **base64url**-encoded without padding, per RFC 7518.

### Privacy

Everything runs **in your browser** via WebAssembly. Your key — including private
keys — is never uploaded to a server. You can also run it from the
[gizza CLI](/) or directly inside a gizza chat.

### Common uses

- Build a JWKS (`{ "keys": [ ... ] }`) for an OIDC/OAuth identity provider.
- Convert an existing TLS or SSH-adjacent key into JWK form for a JOSE library.
- Inspect the raw modulus/exponent or curve point of a key.

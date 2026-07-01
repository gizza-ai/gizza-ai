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

## FAQ

<details>
<summary>Which PEM headers does the converter accept?</summary>

`RSA PRIVATE KEY` and `RSA PUBLIC KEY` (PKCS#1), `EC PRIVATE KEY` (SEC1),
`PRIVATE KEY` (PKCS#8), and `PUBLIC KEY` (SPKI). Anything else — including
`ENCRYPTED PRIVATE KEY` and `OPENSSH PRIVATE KEY` — is rejected with an error
naming the label. Decrypt or re-export such keys to PKCS#8 first (e.g.
`openssl pkcs8 -topk8 -nocrypt`).

</details>

<details>
<summary>I pasted a private key — how do I get the public JWK?</summary>

The private JWK always contains the public members too (`n`/`e` for RSA,
`crv`/`x`/`y` for EC). To publish the public half, copy the JWK and delete the
private members: `d`, `p`, `q`, `dp`, `dq`, `qi` for RSA, or just `d` for EC.

</details>

<details>
<summary>Are Ed25519 or secp256k1 keys supported?</summary>

Not currently. The converter handles RSA and EC keys on the NIST curves
P-256, P-384 and P-521. An EC key on another curve fails with an
"unrecognised curve" style error rather than producing a wrong JWK.

</details>

<details>
<summary>Why are the JWK values not ordinary base64?</summary>

Per RFC 7518, all binary JWK members use **base64url** encoding (`-` and `_`
instead of `+` and `/`) with the trailing `=` padding stripped. If a
consuming library complains, make sure it decodes base64url, not standard
base64.

</details>

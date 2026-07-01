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

## FAQ

<details>
<summary>Do extra members like alg, kid, or use change the thumbprint?</summary>

No. RFC 7638 hashes only the required members for the key type, so `alg`,
`kid`, `use`, `key_ops`, `x5c` and any other metadata are stripped before
hashing. Two JWKs describing the same key material always produce the same
thumbprint, whatever extras they carry and in whatever order.

</details>

<details>
<summary>Which key types are supported?</summary>

`RSA` (hashes `e`, `kty`, `n`), `EC` (`crv`, `kty`, `x`, `y`), `OKP` for
Ed25519/X25519 (`crv`, `kty`, `x`), and `oct` symmetric keys (`k`, `kty`).
Any other `kty` — or a JWK missing one of its required members — returns a
clear error naming what's missing.

</details>

<details>
<summary>Is it safe to paste a private key, and does it give a different thumbprint?</summary>

The key never leaves your browser — the hash is computed locally via
WebAssembly. And for RSA/EC/OKP the required members are the *public*
parameters only (`d`, `p`, `q`, etc. are ignored), so a private JWK yields
exactly the same thumbprint as its public counterpart. Note that for `oct`
keys the secret `k` itself is part of the hash input.

</details>

<details>
<summary>Why is the thumbprint always 43 characters?</summary>

It's the base64url encoding, without padding, of the 32-byte SHA-256 digest —
32 bytes always encode to 43 URL-safe characters. The tool also shows the
exact canonical JSON that was hashed so you can reproduce the digest yourself.

</details>

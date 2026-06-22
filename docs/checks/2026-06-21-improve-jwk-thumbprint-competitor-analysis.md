# jwk-thumbprint — competitor analysis & differentiation

**Tool:** `gizza-ai/jwk-thumbprint` — compute the RFC 7638 SHA-256 thumbprint of a
JSON Web Key (the canonical `kid`).
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `jose`/`jwcrypto`/`node-jose` libs | Library | Have a `thumbprint()` API, but you must write code and install the lib. |
| Online JWK tools | Web | Rare and varied; many **upload your key**, and some compute it wrong (forget to drop optional members or to sort keys). |
| Hand computation | DIY | Easy to get wrong — the spec's canonicalization (only required members, lexicographic order, no whitespace) trips people up. |

## How gizza's tool is better / different

1. **Local — your key never uploaded.** Runs in WASM (chat SW + CLI + page).
2. **Spec-exact (RFC 7638).** Keeps only the required members per key type
   (RSA/EC/OKP/oct), lexicographically ordered, compact, then SHA-256 +
   base64url-nopad — verified against the **canonical RFC 7638 example**
   (`NzbLsXh8uDCcd-6MNwXF4W_7noWXFZAfHkxZsRGC9Xs`).
3. **Shows its work.** Returns the exact **canonical JSON** that was hashed, so
   you can audit the computation — not just the opaque hash.
4. **All four key types** (RSA, EC, OKP/Ed25519, oct), with clear errors for
   missing members or unsupported `kty`.
5. **Three surfaces, one Rust core.**

## Verification

Five core unit tests, including the **RFC 7638 §3.1 RSA vector** (exact match),
plus EC/OKP/oct canonical forms and error cases. The RFC `n` value was fetched
from the RFC to use the authentic vector. **End-to-end CLI** on an oct key
returned the canonical `{"k":...,"kty":"oct"}` and its thumbprint. Page
Playwright re-verifies the RFC RSA vector and the unsupported-kty error.

## Scope / honest limitations

- Computes the SHA-256 thumbprint (the RFC 7638 default / JWK `kid` convention).
  Other hash algs (RFC 9278 "JWK Thumbprint URI" with SHA-384/512) could be a
  future option.
- Members must be present as JSON strings (as in a real JWK).

## Possible future enhancements

- RFC 9278 thumbprint URI output (`urn:ietf:params:oauth:jwk-thumbprint:sha-256:…`).
- Selectable hash (SHA-384/512).
- Accept a full JWK Set and thumbprint each key.

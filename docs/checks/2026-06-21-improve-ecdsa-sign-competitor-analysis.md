# ecdsa-sign — competitor analysis & differentiation

**Tool:** `gizza-ai/ecdsa-sign` — sign a message with an ECDSA private key (NIST
P-256 or P-384), outputting the signature in DER or raw r||s form.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `openssl dgst -sign` | CLI | The reference, but multi-step (hash + sign), DER-only by default, no raw r||s without extra conversion; needs OpenSSL installed and a temp file for the message. |
| Online "ECDSA signature" tools (e.g. lapo.it, 8gwifi.org, devglan) | Web | Most **upload your private key to a server** — unacceptable for a signing key. Many only verify, or only do P-256. |
| `jsrsasign` / WebCrypto playgrounds | Web/lib | Require writing JS; WebCrypto only emits raw (r||s), not DER, and key import is fiddly (PKCS#8 ArrayBuffer juggling). |
| Python `cryptography`, Node `crypto` | Library | Need code; encoding choice (DER vs raw) is a non-obvious API detail people get wrong. |

## How gizza's tool is better / different

1. **Key never leaves the device.** The whole thing runs in WASM locally (chat
   service worker, CLI, or browser page). Signing keys are the one thing you must
   never paste into a remote box — most online competitors fail this outright.
2. **Both encodings, one toggle.** DER (OpenSSL/X.509/TLS) **and** raw r||s
   (JOSE `ES256`/`ES384`, WebCrypto) — no manual ASN.1 ↔ P1363 conversion.
3. **Deterministic (RFC-6979).** Same key+message → same signature, every time.
   No RNG, so no catastrophic nonce-reuse risk, and signatures are testable/
   reproducible.
4. **Right hash, automatically.** The curve fixes the digest (P-256→SHA-256,
   P-384→SHA-384), so users can't accidentally mismatch curve and hash.
5. **base64 *and* hex output** plus the byte length, so it drops straight into
   whichever tool you're feeding next.
6. **Three surfaces.** Chat (`sign this with my key`), CLI (`gizza tool
   ecdsa-sign`), and a zero-upload web page — same Rust core.

## Verification

The CLI output was independently verified with OpenSSL
(`openssl dgst -sha256 -verify pub.pem -signature sig.der msg`) → `Verified OK`,
confirming the DER signatures are standards-correct and interoperable.

## Scope / honest limitations

- **PKCS#8 PEM keys only** (`-----BEGIN PRIVATE KEY-----`). SEC1 `EC PRIVATE KEY`
  keys must be converted first (`openssl pkcs8 -topk8 -nocrypt`); the error
  message says so.
- **P-256 and P-384 only.** P-521 was deliberately excluded: the `p521` crate's
  ECDSA path is randomized-only (no RFC-6979) and pulls a getrandom dependency,
  which would break determinism and complicate the WASM build. P-256/P-384 cover
  the overwhelming majority of ECDSA usage (TLS, JOSE ES256/ES384, WebAuthn).

## Possible future enhancements

- Accept SEC1 keys directly (needs the `sec1` decode path wired up).
- Add an Ed25519 sibling tool (EdDSA) for the non-NIST audience.

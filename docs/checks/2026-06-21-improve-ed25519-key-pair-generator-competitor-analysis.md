# ed25519-key-pair-generator — competitor analysis & differentiation

**Tool:** `gizza-ai/ed25519-key-pair-generator` — generate an Ed25519 key pair,
output public and private keys in standard encodings.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `openssl genpkey -algorithm ed25519` / `ssh-keygen -t ed25519` | CLI | The references, but need OpenSSL/OpenSSH installed; output a single encoding (PEM or OpenSSH), not raw base64/hex alongside; multi-step to also get the public key separately. |
| Online "Ed25519 key generator" sites | Web | Many **generate the private key on a server** — a non-starter for key material. Quality varies; some only emit hex, others only OpenSSH. |
| `tweetnacl` / WebCrypto playgrounds | Web/lib | Need to write JS; WebCrypto only recently added Ed25519 and browser support is uneven; PKCS#8 export is fiddly. |
| Language libs (`cryptography`, Go `crypto/ed25519`, `libsodium`) | Library | Require code; each picks its own default encoding, so interop needs manual conversion. |

## How gizza's tool is better / different

1. **Generated locally — key never touches a server.** Runs in WASM in the chat
   service worker or the CLI, using `getrandom` (WASI `random_get`) for the
   CSPRNG. The single most important property for a key generator, and where most
   web competitors fail.
2. **Every encoding at once.** One call returns the private key as **PKCS#8 PEM**,
   the public key as **SPKI PEM**, *and* both raw 32-byte keys in **base64** and
   **hex** — so it drops straight into OpenSSL, JOSE/JWT (`EdDSA`), libsodium,
   or any language lib without conversion.
3. **Modern, fast curve.** Ed25519 (EdDSA) is the recommended default for new
   systems — small keys, fast signing, deterministic, no parameter choices to get
   wrong (unlike RSA key-size or ECDSA curve/hash selection).
4. **Verified correct.** The core test signs and verifies a message under the
   generated pair; the CLI output was independently parsed by `openssl pkey`
   (recognized as a valid `ED25519 Private-Key`).

## Surfaces & honest scope

- **Chat + CLI only — no web page.** Like `generate-rsa-key-pair`, this is a
  zero-input, non-deterministic generator, which doesn't fit the page's
  recompute-on-input model (there is no field to drive, and a page would emit one
  fixed key on load). Key generation belongs in chat ("generate me an Ed25519
  key") or the CLI (`gizza tool ed25519-key-pair-generator`).

## Possible future enhancements

- Optional OpenSSH `authorized_keys` / private-key format output.
- Sibling `ed25519-sign` / `ed25519-verify` tools (pairs naturally with this and
  with the existing `ecdsa-sign` / `rsa-sign`).

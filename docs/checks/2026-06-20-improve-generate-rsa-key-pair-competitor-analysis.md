# generate-rsa-key-pair — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/generate-rsa-key-pair` — generate an RSA key pair (2048 /
3072 / 4096 bits) and return the private key (PKCS#8 PEM) and public key (SPKI
PEM). Chat + CLI (no page: RSA key generation is heavy and non-deterministic,
which doesn't fit the page's live-recompute-on-input model).

## What competitors do

- **Online RSA key generators** (cryptotools.net, devglan, travistidwell RSA,
  8gwifi) — pick a size, get PEM keys in the browser. Strength: convenient.
  Weakness: **the keys are often generated on a server** (or it's unclear), which
  is a serious trust problem for *private* keys — you should never accept a
  private key a remote server has seen. Some also cap sizes or add ads.
- **`openssl genpkey` / `ssh-keygen`** — the reference, fully local, but require
  a shell and remembering flags.
- **Language libs** (Python `cryptography`, Node `crypto.generateKeyPair`) — need
  a runtime + code.

## How this tool competes / improves

1. **Generated locally — the private key never touches a server.** Pure-Rust
   (`rsa` crate) compiled to wasm: it runs in the chat Service Worker and headless
   in the CLI, seeded by a cryptographic RNG (getrandom → WASI `random_get`). This
   is the single most important property for a keygen tool and the one most online
   generators get wrong.
2. **Standard, ready-to-use formats.** Private key as **PKCS#8 PEM**
   (`-----BEGIN PRIVATE KEY-----`) and public key as **SPKI PEM**
   (`-----BEGIN PUBLIC KEY-----`) — the formats OpenSSL, ssh, JWT libraries, and
   most tooling expect, no conversion needed.
3. **Real sizes, free.** 2048 / 3072 / 4096 with no paywall or size gate.
4. **Verifiable.** The output validates with `openssl rsa -check`, and the
   returned public key is exactly what OpenSSL derives from the private key
   (cross-checked — see tests).

## Honest scope / notes

- RSA only (this tool's remit). Ed25519/ECDSA would be separate tools.
- No passphrase-encryption of the private key yet (it's emitted unencrypted
  PKCS#8); pair with the existing `encrypt-file` tool if you need an encrypted
  blob, or this could gain a `passphrase` option later.
- 4096-bit generation is CPU-heavy (can take several seconds), which is the main
  reason there's no live page surface; 2048 is the fast default.

## Tests

3 core unit tests: `validate_bits` accepts 2048/3072/4096 and rejects others; a
full 2048 generation produces a PKCS#8 private PEM + SPKI public PEM, the private
PEM **re-parses** and its modulus is exactly 2048 bits, and the derived public
PEM matches the returned one; two generations differ (fresh randomness). Plus the
block drift-guard schema test. The `rsa` crate was confirmed to compile to
`wasm32-wasip1`. CLI verified end-to-end and **cross-validated with OpenSSL**:
`openssl rsa -check` reports "RSA key ok", and `openssl rsa -pubout` on the
private key reproduces the returned `public_pem` byte-for-byte.

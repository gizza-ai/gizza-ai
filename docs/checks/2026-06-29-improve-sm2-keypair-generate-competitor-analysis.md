# sm2-keypair-generate — competitor analysis & differentiation

**Tool:** `gizza-ai/sm2-keypair-generate` — generate an SM2 key pair (Chinese
national standard GM/T 0003, OSCCA curve `sm2p256v1`), output the public and
private keys in standard encodings.
**Date:** 2026-06-29

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `openssl ecparam -name SM2 -genkey` / `openssl genpkey -algorithm SM2` | CLI | The reference, but needs a recent OpenSSL (≥1.1.1 / 3.x) built with SM2 support; emits a single encoding (PEM); getting the raw scalar / public point in hex is a separate `-text` parse step; multi-step to also export the public key. |
| GmSSL CLI / Tongsuo (BabaSSL) | CLI | Authoritative SM2/SM3/SM4 toolkits, but a heavyweight install most people don't have; primarily PEM/DER output, no one-shot raw-hex bundle. |
| Online "SM2 key generator" sites | Web | Several **generate the private key on a server** — a non-starter for key material. Many are Chinese-language only; quality and encoding choices vary (some emit only the raw hex scalar, others only PEM). |
| `gmssl`/`sm-crypto` (JS), `gmssl` (Go), `cryptography`+`gmssl` (Python) | Library | Require writing code; each library picks its own default encoding (raw hex vs PKCS#8 vs DER), so interop needs manual conversion; browser libs vary in CSPRNG quality. |

## How gizza's tool is better / different

1. **Generated locally — the key never touches a server.** Runs in WASM in the
   chat service worker or the CLI, using `getrandom` (WASI `random_get`) for the
   CSPRNG. The single most important property for a key generator, and where most
   web competitors fail.
2. **Every encoding at once.** One call returns the private key as **PKCS#8 PEM**,
   the public key as **SPKI PEM**, the raw **32-byte scalar in hex**, and the
   public point in **SEC1 hex** — both **uncompressed** (`04 || x || y`, 65 B) and
   **compressed** (`02|03 || x`, 33 B). It drops straight into OpenSSL, GmSSL, or
   any language lib without conversion.
3. **Standards-conformant, verified correct.** The OSCCA curve identifier is
   carried in the PEM, and OpenSSL independently parses both the private and
   public PEM and reports `ASN1 OID: SM2` with a matching public point. The core
   tests re-parse the private PEM and confirm the derived public key and scalar
   match the reported encodings, and that two generations differ.
4. **No toolchain to install.** Unlike `openssl`/GmSSL/Tongsuo (which need a build
   with SM2 enabled), this works from chat or a single `gizza tool` call.

## Surfaces & honest scope

- **Chat + CLI only — no web page.** Like `ed25519-key-pair-generator`,
  `generate-rsa-key-pair`, and `generate-pgp-key-pair`, this is a zero-input,
  non-deterministic generator, which doesn't fit the page's recompute-on-input
  model (there is no field to drive, and a page would emit one fixed key on load).
  Key generation belongs in chat ("generate me an SM2 key") or the CLI
  (`gizza tool sm2-keypair-generate`).

## Possible future enhancements

- Optional DER (binary) output alongside PEM.
- Sibling `sm2-encrypt` / `sm2-sign` / `sm2-verify` tools (pair naturally with
  this and with the existing `ecdsa-sign` / `rsa-sign` family); the RustCrypto
  `sm2` crate exposes `dsa` (signing) and `pke` (encryption) modules.

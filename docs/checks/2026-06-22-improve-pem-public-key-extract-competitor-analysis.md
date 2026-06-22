# pem-public-key-extract — competitor analysis (2026-06-22)

Tool: derive the **public** key (PEM SubjectPublicKeyInfo, `-----BEGIN PUBLIC KEY-----`)
from a **private** key. Offline in-browser equivalent of `openssl pkey -in key.pem -pubout`.

## Surfaces verified

- **Chat block** — `wafer build` validates `target/block.wasm` instantiates in wasm32-wasip1
  (all crypto crates — `rsa`, `p256`, `p384`, `ed25519-dalek` — instantiate cleanly).
- **CLI** — `gizza tool pem-public-key-extract` cross-checked byte-for-byte against
  `openssl pkey -pubout` for RSA-2048, EC P-256, and Ed25519 private keys (PKCS#8 + SEC1).
- **Page** — Playwright `tool-page-pem-public-key-extract.spec.ts`, 2 tests green (form fill
  + deep-link query-params), output matches the openssl-derived public key.

## Competitor landscape

| Competitor | Key types | Input forms | Output | Privacy |
|---|---|---|---|---|
| 8gwifi.org (Extract Public Key) | RSA, DSA, EC | PEM private key | PEM public key | server-side (key uploaded) |
| FYIcenter Key Decoder/Viewer | RSA, EC, etc. | PEM | decoded view (not just public PEM) | server-side |
| IPparse Public Key Extractor | RSA/EC | PEM | PEM public key | server-side |
| `openssl pkey -pubout` (CLI) | RSA, EC, Ed25519, DSA | PEM/DER | PEM/DER public key | local |
| **gizza pem-public-key-extract** | RSA, EC (P-256/P-384), Ed25519 | PEM (PKCS#8/PKCS#1/SEC1) + raw DER (hex/base64) | PEM SPKI public key | **100% in-browser, never uploaded** |

## Gap diff + ranking (fit-to-model)

Capabilities closed / already covered:

1. **Privacy (top differentiator).** Every web competitor uploads the private key to a server —
   a serious anti-pattern for secret key material. gizza runs entirely client-side via wasm; the
   key never leaves the machine. Already in copy (hero + content.md). **Done.**
2. **Ed25519 support.** 8gwifi/IPparse cover RSA/DSA/EC but NOT modern Ed25519 (a common gap that
   only openssl 3.x closes). gizza supports it. **Done.**
3. **Multiple private-key encodings.** gizza accepts PKCS#8 (`PRIVATE KEY`), PKCS#1
   (`RSA PRIVATE KEY`), and SEC1 (`EC PRIVATE KEY`) PEM, plus raw DER as hex/base64 — broader than
   the PEM-only competitors. **Done.**
4. **Auto-detect.** `key_type=auto` reads the PEM label and otherwise tries each algorithm, so the
   user rarely has to pick — matches the zero-config feel of the online tools. **Done.**
5. **Standard SPKI output.** Output is always the universal `-----BEGIN PUBLIC KEY-----` SPKI form
   (drops into OpenSSL/JWT/TLS), matching `openssl pkey -pubout`. **Done.**

## Out-of-model / deliberately not built

- **DSA keys.** 8gwifi lists DSA, but DSA is deprecated and the maintained pure-Rust `dsa` crate
  pulls a randomized signer path; the cost/value is poor for a near-dead algorithm — skipped.
- **EC curves beyond P-256/P-384** (P-521, secp256k1, Curve25519-as-X25519). P-521's pure-Rust
  ECDSA support is randomized-only (see SKILL findings) and the others are uncommon for this
  conversion. P-256/P-384 cover the overwhelming majority of EC keys in the wild.
- **Encrypted (passphrase-protected) private keys.** Would need a passphrase param + PKCS#8
  decryption path; out of scope for v1 (most extract-pubkey flows use unencrypted keys).
- **Full key *decoding/viewing*** (modulus, exponent, curve params à la FYIcenter). That is a
  distinct "inspect a key" tool, not "extract the public key"; not merged to keep this tool focused.
- **Public-key DER output / fingerprint.** Output is PEM only for now (the canonical interchange
  form); DER/hex output would be additive but is a separate concern.

No competitor copy, branding, or trademarks were used.

## Sources

- [8gwifi.org — Extract Public Key from Private Key](https://8gwifi.org/pempublic.jsp)
- [8gwifi.org — PEM Parser](https://8gwifi.org/PemParserFunctions.jsp)
- [FYIcenter — Public/Private Key Decoder and Viewer](https://certificate.fyicenter.com/2145_FYIcenter_Public_Private_Key_Decoder_and_Viewer.html)
- [IPparse — Public Key Extractor](https://ipparse.com/public-key-extractor)
- [OpenSSL discussion — derive Ed25519 public key](https://github.com/openssl/openssl/discussions/21865)
- [SysTutorials — extracting EC public keys with OpenSSL](https://www.systutorials.com/extracting-ec-public-keys-with-openssl/)

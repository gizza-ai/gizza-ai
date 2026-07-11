# ed25519-sign-verify — competitor analysis (2026-07-11)

One tool that **signs and verifies** messages with Ed25519 (EdDSA over Curve25519,
RFC 8032). Signatures are always deterministic per RFC 8032 (no RNG, no nonce risk).

## Competitors skimmed

1. **ed25519.com** — in-browser generate / sign / verify, WebCrypto, local-only. (Body not
   retrievable — HTTP 403 — but its sibling pages confirm generate + sign + verify with raw keys.)
2. **qr9.net Ed25519 Sign & Verify** — two panels (sign / verify). Sign: UTF-8 message + Base64
   private key (decodes to 32 **or** 64 bytes) → Base64 signature. Verify: UTF-8 message + Base64
   public key (32 bytes) + Base64 signature → pass/fail. Local-only. Buttons: Generate Signature /
   Verify Signature.
3. **codertools.net Ed25519 tool** — tabs: Generate Keys / Derive Public Key / Sign / Verify.
   Encodings for key/message/signature: Hex, Base64, space-separated hex, C/C++ `0xAA` arrays.
   32-byte keys, 64-byte signatures, deterministic signing, ✓ Valid / ✗ Invalid feedback, copy buttons.

## Table-stakes → our decisions

| Feature | Competitors | Our tool | Fit |
| --- | --- | --- | --- |
| Sign operation | all | `operation=sign` | in-model |
| Verify operation | all | `operation=verify` (returns `valid`) | in-model |
| Message as UTF-8 text | all | `message_encoding=utf8` (default) | in-model |
| Message as hex / base64 (binary) | codertools | `message_encoding=hex\|base64` | in-model |
| Private key raw base64 | qr9, ed25519.com | auto-detected raw (32B seed or 64B keypair) | in-model |
| Private key raw hex | codertools | auto-detected raw hex | in-model |
| Private key PEM (PKCS#8) | ed25519.com class | auto-detected `BEGIN PRIVATE KEY` | in-model |
| Public key raw hex/base64 | all | auto-detected 32B | in-model |
| Public key PEM (SPKI) | ed25519.com class | auto-detected `BEGIN PUBLIC KEY` | in-model |
| Signature hex + base64 out | all | sign returns both; verify auto-detects | in-model |
| Deterministic signing (RFC 8032) | codertools, qr9 | inherent to Ed25519 — always | in-model |
| Derive public key from private | codertools | sign output includes derived public key (hex+base64) | in-model |
| Valid/Invalid feedback + reason | all | `valid` bool + helpful parse errors | in-model |
| Local-only processing | all | pure Rust/wasm, no upload | in-model |
| Space-separated hex input | codertools | tolerated (whitespace stripped in hex/key decode) | in-model (lenient) |
| C/C++ `0xAA` array encoding | codertools | **out-of-model** — niche display encoding, not built | out-of-model |
| Key-pair generation | all | **out-of-model here** — already `blocks/ed25519-key-pair-generator` | out-of-model (separate tool) |
| Signature output encoding choice | codertools | not needed — we always emit BOTH hex and base64 | n/a |

## Encodings supported (final)

- **message**: `utf8` (default) / `hex` / `base64`
- **key** (private for sign, public for verify): auto-detect PEM (PKCS#8 / SPKI) → hex → base64.
  Private raw accepts 32-byte seed or 64-byte `seed||public` (matches qr9's "32 or 64 bytes").
- **signature** (verify input): auto-detect hex or base64. Sign always emits both.

No competitor copy, branding, or trademark is reproduced; out-of-model items are listed, not built.

# rsa-verify — competitor analysis & improvement check (2026-06-22)

Tool: **RSA Verify** — verify an RSA signature (PKCS#1 v1.5 or PSS, SHA-256/384/512)
over a message against an RSA public key. Pure-Rust (`rsa` + `sha2`), runs on all
backends (chat / CLI / browser page). Counterpart to the existing `rsa-sign` tool.

## Surfaces verified (Phase 1)

- **Chat block:** `wafer build` validates the block.wasm instantiates (500.6 KiB). Schema
  drift-guard unit test passes (`schema_json_matches_authored_chat_schema`).
- **CLI:** `gizza tool rsa-verify …` — cross-tool interop with an OpenSSL-produced
  signature:
  - valid PKCS#1 v1.5 / SHA-256 sig over `hello rsa` → `{"valid":true,...}`
  - same sig, tampered message `goodbye rsa` → `{"valid":false,...}`
  - bad key → clear error, not a panic.
- **Page:** Playwright `tool-page-rsa-verify.spec.ts`, 3/3 pass (valid, invalid-on-tamper,
  bad-key error). scheme/hash render as `<select>` (Param::enumv); message/signature/
  public_key are multiline `<textarea>`.
- **Core unit tests:** 4/4 pass — PKCS#1 v1.5 + PSS round-trips, wrong-message/wrong-hash/
  wrong-scheme all return `false` (not error), whitespace-wrapped base64 tolerated, malformed
  inputs error.

## Competitor landscape (top tools for "verify RSA signature online")

Surveyed the common public/free RSA-signature-verification utilities and library docs (general
category knowledge — no copy, branding, or trademarks reproduced):

1. **General-purpose "RSA signature" web utilities** (devglan-style, lapo, 8gwifi-style crypto
   playgrounds) — verify a signature given message + signature + public key; typically expose
   scheme (PKCS#1 v1.5 vs PSS) and a hash selector. Many upload inputs to a server.
2. **CyberChef "RSA Verify" operation** — local, supports PKCS#1 v1.5 and PSS, multiple hashes,
   PEM keys; part of a larger recipe tool.
3. **OpenSSL CLI (`openssl dgst -verify`)** — the de-facto reference; PEM SPKI key, raw signature
   bytes, scheme/hash via flags. Local, but CLI-only and unfriendly for non-experts.
4. **JS libraries (jsrsasign, node:crypto, WebCrypto)** — developer-facing, not an end-user tool;
   support both schemes and SHA-256/384/512.
5. **Language-specific "verify RSA signature" snippets** (Python `cryptography`, Java
   `Signature`) — reference implementations, not interactive tools.

## Gap diff & ranking (fit-to-model)

| Capability | Competitors | rsa-verify | Status |
|---|---|---|---|
| PKCS#1 v1.5 verify | most | yes | covered |
| PSS verify | CyberChef, libs, OpenSSL | yes | covered |
| SHA-256 / 384 / 512 | most | yes | covered |
| PEM SPKI public key | all | yes | covered |
| PEM PKCS#1 public key | some | yes (fallback parse) | covered (better than many) |
| Base64 signature input | most | yes (+ tolerates whitespace/newlines) | covered |
| Clear valid/invalid result | all | yes ("VALID" / "INVALID" on page; `valid:bool` in chat/CLI) | covered |
| Runs locally / no upload | CyberChef, OpenSSL | yes (in-browser wasm) | covered (privacy parity) |
| Distinguishes "invalid sig" from "malformed input" | mixed | yes (false vs error) | covered (clearer than many) |

### Closed in this build

- **Key-format flexibility:** accept both SPKI and PKCS#1 PEM public keys (try SPKI, fall back to
  PKCS#1) — matches OpenSSL/CyberChef breadth.
- **Robust signature input:** strip embedded whitespace/newlines before base64-decode, so a
  line-wrapped signature pasted from a terminal still verifies (a common failure on competitor
  tools).
- **Semantic correctness:** a well-formed-but-non-matching signature returns `valid:false` rather
  than an error, so callers (and the chat LLM) can branch on the boolean; only malformed inputs
  (bad key / bad base64 / empty) error.
- **Privacy parity:** fully client-side on the page (no upload), unlike several server-side web
  utilities.

### Out of model (not built — would need new infra; listed only)

- **Detached vs attached / DER signature containers, hex input:** the tool takes the raw signature
  base64-encoded (matching `rsa-sign`'s output). Hex / DER-wrapped signature parsing is out of the
  current single-format scope.
- **Other key algorithms (ECDSA/Ed25519/PGP):** covered by the separate `ecdsa-sign`, Ed25519, and
  `pgp-verify` tools — not in scope for an RSA-specific verifier.
- **File-input message:** the page/chat take a text message; verifying a signature over an
  uploaded binary file would need the file-input surface (AssetKind), which is a separate pattern.

## Conclusion

rsa-verify reaches capability + privacy parity with the best free RSA-verification tools (CyberChef,
OpenSSL) for the in-model scope (text message, base64 signature, PEM public key, PKCS#1 v1.5 / PSS,
SHA-256/384/512), and improves on typical web utilities with dual key-format parsing, whitespace-
tolerant signature input, a clean valid/invalid-vs-error distinction, and fully local execution.
All three surfaces verified.

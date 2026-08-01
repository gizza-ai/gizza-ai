# hmac-verify — competitor analysis (2026-07-30)

Tool: constant-time verification of an HMAC tag against a message and secret key.
Sibling to the existing `hmac-generate` (which produces tags). This tool checks a
supplied tag and reports MATCH / MISMATCH using a timing-safe comparison.

## Top competitors scanned (paraphrased, no copy reused)

1. **CodeShack HMAC Generator** (codeshack.io/hmac-generator) — generate mode plus
   a "compare"/expected-signature box: paste the signature a provider sent and it
   reports instantly whether it matches. Client-side (Web Crypto). Algorithms:
   SHA-256/384/512, SHA-1, MD5. Hex output.

2. **WebhookRelay HMAC Generator & Verifier** (webhookrelay.com/hmac-verification) —
   paste payload + secret to generate, OR check a computed hash against the
   signature a provider sent. Algorithms SHA256/SHA512/SHA1/MD5. Framed around
   verifying GitHub / Stripe / Shopify / Slack webhooks.

3. **Devglan Generate/Verify HMAC** (devglan.com/online-tools/hmac-sha256-online) —
   verify mode recalculates the MAC and "securely compares" it with the provided
   value. SHA-256/384/512. Aimed at API auth, webhook signatures, integrity checks.

Also seen: Authgear (SHA-256/384/512, hex/base64, client-side), AquilaX,
100Plus / PulpMiner webhook verifiers (SHA256 default, SHA512, SHA1; hex or base64;
Stripe/GitHub/Twilio framing).

## Table-stakes (params / defaults / UX)

| Capability | Competitors | Decision for gizza |
|---|---|---|
| message + secret key inputs | all | IN — `message`, `key` |
| expected tag / signature paste box → MATCH/MISMATCH | CodeShack, WebhookRelay, Devglan | IN — `expected` param, boolean-ish MATCH/MISMATCH report |
| algorithm select, SHA-256 default | all | IN — `algorithm` enumv, default sha256; also sha1/224/384/512, sha3-256/512, md5 (superset of competitors, matches hmac-generate) |
| hex OR base64 tag encoding | Authgear, 100Plus | IN — `expected_encoding` enumv (hex/base64/auto). Auto-detect by trying hex then base64 so pasting either form just works |
| key as text/hex/base64 (binary keys) | Devglan (base64 key), general | IN — `key_encoding` enumv (matches hmac-generate) |
| message as text/hex/base64 | hmac-generate parity | IN — `message_encoding` enumv |
| timing-safe / "secure compare" | Devglan, general best practice | IN — RustCrypto `Mac::verify_slice` (constant-time); no early-exit compare |
| client-side / privacy | all | IN — pure Rust/wasm, nothing leaves the browser |
| webhook framing (Stripe/GitHub) | WebhookRelay, 100Plus | IN — FAQ + content examples; strip `sha256=`/`v1=` prefixes noted |

## In-model vs out-of-model

- IN-MODEL (built): message/key/expected inputs, 8 algorithms, message/key encodings,
  expected-tag encoding with hex-or-base64 auto-detect, constant-time verify, a
  report showing status + computed vs expected tag, hex prefix tolerance (`0x`),
  webhook signature-prefix guidance.
- OUT-OF-MODEL (not built, no server/model here): live webhook-provider presets that
  auto-select the header format per provider; multi-line signature-header parsing
  (e.g. Stripe's `t=…,v1=…` composite header) — the tool verifies a single tag, and
  the FAQ tells the user which substring to paste.

No competitor copy, branding, or trademarked text was copied; findings paraphrased.

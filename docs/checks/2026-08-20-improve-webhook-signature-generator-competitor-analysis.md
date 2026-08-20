# webhook-signature-generator — competitor analysis (2026-08-20)

Scan run before finishing the tool. Notes are paraphrased; no competitor copy, branding, or assets were reused.

## Competitors reviewed

| # | Competitor | Shape | Relevant table stakes |
|---|---|---|---|
| 1 | Stripe webhook signature docs / CLI examples | Provider documentation and CLI replay workflow | Signs `<timestamp>.<raw_body>` with HMAC-SHA256 over the endpoint secret, returns a `Stripe-Signature` header containing `t=` and `v1=` fields, and relies on current or captured Unix timestamps for replay. |
| 2 | GitHub webhook validation docs | Provider documentation | Uses HMAC-SHA256 over the raw body with a shared secret and sends `X-Hub-Signature-256: sha256=<hex>`, with a legacy SHA-1 header still documented for older integrations. |
| 3 | Slack request signing docs | Provider documentation | Signs `v0:<timestamp>:<raw_body>` with HMAC-SHA256 and emits separate timestamp and signature headers. The timestamp is mandatory and replay-window checking is expected. |
| 4 | Standard Webhooks / Svix signing docs | Open standard and provider docs | Signs `<id>.<timestamp>.<raw_body>`, base64-decodes `whsec_` secret material, and emits id, timestamp, and `v1,<base64>` signature headers. |
| 5 | Generic HMAC / webhook signature helper snippets | Small web calculators and code snippets | Usually expose algorithm, encoding, secret format, signed-string template, and header prefix, but do not know provider-specific canonical strings or multi-header layouts. |

## Table stakes shipped

- Raw payload input with warnings that exact bytes matter.
- Secret input plus explicit `secret_encoding` (`auto`, `text`, `hex`, `base64`) so encoded keys do not get silently misused.
- Provider presets for Stripe, GitHub, Slack, Shopify, Standard Webhooks, Svix, Square, Twilio, Paddle, and Custom.
- Timestamp parameter with Unix seconds and ISO-8601 support; millisecond timestamps are rejected with an actionable error.
- Message id support for Standard Webhooks/Svix and URL support for Square/Twilio.
- Named-provider canonical strings, algorithms, encodings, and header names fixed to the provider scheme.
- Custom HMAC mode with template placeholders, algorithm enum, encoding enum, header name, and signature prefix.
- Output selector for full report, all headers, primary header value, bare signature, signed payload, or replay cURL.
- Published-provider unit vectors for GitHub, Slack, Standard Webhooks/Svix, Twilio, plus deterministic tests for other providers and custom mode.

## Considered, not built

- Asymmetric signatures such as SendGrid ECDSA, PayPal certificate signatures, and Discord Ed25519. Those are not HMAC schemes and need public/private key verification workflows rather than shared-secret generation.
- Sending the webhook request from the page. The generic tool page should not perform cross-origin POSTs with user secrets; it emits a cURL command instead.
- Constant-time verification of incoming headers. This tool generates replay headers; verification belongs in server code with timestamp windows and replay protection.
- Secret storage or provider account integration. All inputs are pasted per run and stay local.
- Multipart/body canonicalization helpers. Providers sign the bytes they send, so mutation helpers would risk generating misleading signatures.

## Verification snapshot

Built and verified on 2026-08-20 with focused cargo tests, canonical wasm build, wasm-pack web build, manifest sync, CLI exact-output checks, Playwright page tests, JavaScript smoke tests, and `scripts/check-tool-hygiene.py webhook-signature-generator` before commit.

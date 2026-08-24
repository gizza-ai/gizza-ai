## About this tool

Webhook Signature Generator builds the HMAC headers that webhook providers send with a delivery. Paste the exact raw body, the endpoint's signing secret, and a fixed timestamp when the provider uses one; the tool returns the signed byte string, the encoded HMAC, finished header lines, and an optional replay `curl` command.

A worked Stripe example:

1. Payload: `{"id":"evt_test","type":"payment_intent.succeeded"}`
2. Secret: `whsec_test_secret`
3. Provider: `stripe`
4. Timestamp: `1700000000`
5. Output: `headers`

The primary result is a `Stripe-Signature` header in the `t=1700000000,v1=<hex hmac>` format. Switch the provider to GitHub, Slack, Shopify, Standard Webhooks, Svix, Square, Twilio, or Paddle to use that provider's canonical string-to-sign and header layout.

Use `custom` when a webhook source only says "HMAC this string". The custom mode supports `{payload}`, `{timestamp}`, `{id}`, and `{url}` placeholders, selectable HMAC algorithms, hex/base64 encodings, and your own header name/prefix.

Limits and edge cases:

- The payload cap is 2 MiB so browser and Service Worker runs stay responsive.
- The payload is signed exactly as pasted. JSON whitespace, key order, newline style, and form encoding are part of the signature.
- Timestamped providers accept Unix seconds or ISO-8601 input. Millisecond timestamps are rejected with a hint.
- Standard Webhooks and Svix `whsec_` secrets are base64-decoded in `auto` mode; Stripe `whsec_` secrets are literal text.
- This tool generates signatures for testing your own endpoint. It does not verify an incoming request and it does not send traffic unless you copy and run the cURL command yourself.
- Asymmetric providers such as SendGrid ECDSA, PayPal certificates, and Discord Ed25519 are outside this HMAC-only model.

## FAQ

<details>
<summary>Why does changing JSON formatting change the signature?</summary>

Webhook providers sign the raw request body bytes, not the parsed JSON object. A pretty-printed JSON document, compact JSON document, or reordered object can all represent the same data while producing different bytes and therefore different HMACs. Paste the body exactly as your receiver sees it.

</details>

<details>
<summary>What timestamp should I use?</summary>

Use the timestamp from a real delivery when you want to reproduce or debug a mismatch. Leave it blank when generating a fresh replay header for providers such as Stripe or Slack; the page and CLI fill the current Unix seconds. Receivers usually reject old timestamps, so a historical signature may need a test-mode bypass on your endpoint.

</details>

<details>
<summary>Why are Standard Webhooks and Svix secrets decoded but Stripe secrets are not?</summary>

Standard Webhooks and Svix define `whsec_` as a prefix before base64 key material, so `auto` strips the prefix and decodes the rest. Stripe's `whsec_...` value is used as literal text for the HMAC key. Mixing those conventions is a common source of signatures that look valid but never verify.

</details>

<details>
<summary>Can this verify incoming webhook requests?</summary>

No. It is a generator for replay and endpoint tests. To verify a request, compute the same signature with the raw body and compare it to the incoming header using constant-time comparison in your server code, along with provider-specific timestamp and replay checks.

</details>

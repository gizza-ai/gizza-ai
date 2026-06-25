## About this tool

This offline JSON Web Token (JWT) decoder allows you to safely parse, inspect, and validate compact JWTs entirely client-side. The tool uses a locally compiled WebAssembly (WASM) binary to decode standard base64url-encoded parts (Header and Payload) and perform validation on standard claims. Since it runs fully in your browser, **your tokens never leave your device**, making it completely secure for sensitive developer keys, identity tokens, and access tokens.

---

### Standard Claims Explained

JWT payloads typically include claim definitions that assert facts about the token's subject, issuer, and lifetime. Here is a guide to the most common standard claims:

| Claim | Full Name | Purpose & Validation Rules |
| :--- | :--- | :--- |
| **`exp`** | Expiration Time | The timestamp after which the token must be rejected. The decoder checks if the current time is less than `exp` (plus any allowed clock skew leeway). |
| **`nbf`** | Not Before | The timestamp before which the token must not be accepted. The decoder checks if the current time is greater than or equal to `nbf`. |
| **`iat`** | Issued At | The timestamp when the token was created. It is used to identify the age of the token and detect anomalies (e.g. issued in the future). |
| **`iss`** | Issuer | Identifies the security principal that issued the JWT. |
| **`aud`** | Audience | Identifies the recipients that the JWT is intended for. |
| **`sub`** | Subject | Identifies the subject of the JWT (e.g. user ID). |

---

### Frequently Asked Questions

<details class="tool-faq">
<summary>Does this tool verify the cryptographic signature of the token?</summary>
<div class="faq-content">
No. This tool is designed purely for <strong>decoding and inspecting</strong> the structure and claims of the token offline without requiring public keys or secrets. To cryptographically verify that a token has not been tampered with, use the companion <a href="/tools/jwt-verify/">JWT Verify Tool</a>.
</div>
</details>

<details class="tool-faq">
<summary>How is my token kept private?</summary>
<div class="faq-content">
All base64url decoding and JSON formatting are executed locally inside WebAssembly (compiled from Rust) on your browser. No data is transmitted to our servers or third-party APIs. You can even run this page entirely offline.
</div>
</details>

<details class="tool-faq">
<summary>What is clock leeway?</summary>
<div class="faq-content">
Clock leeway is a configurable skew tolerance (in seconds) to account for slight differences between the server clock that generated the token and the client machine decoding it. For example, if a token expired 2 seconds ago, a clock leeway of 5 seconds will treat the token as still valid.
</div>
</details>

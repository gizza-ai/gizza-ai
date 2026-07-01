## About this tool

**JWT sign** builds a **JSON Web Token** (JWT — the JWS compact serialization)
from your **payload** (the claims set) and an optional **header**, then signs it
with the algorithm you choose. The result is the familiar
`header.payload.signature` string that you can hand to an API or store in a
session.

- **HMAC (shared secret):** `HS256`, `HS384`, `HS512` — sign with any secret
  string. The same secret verifies the token.
- **RSA (private key):** `RS256`, `RS384`, `RS512` — RSASSA-PKCS#1 v1.5. Paste a
  PEM private key (PKCS#8 `-----BEGIN PRIVATE KEY-----` or PKCS#1
  `-----BEGIN RSA PRIVATE KEY-----`); verify with the matching public key.
- **ECDSA (private key):** `ES256` (P-256), `ES384` (P-384) — JWS raw `r‖s`
  signatures. Paste a PEM PKCS#8 EC private key.

The `alg` header is always set from the algorithm you pick, and `typ` defaults to
`JWT` unless your header overrides it.

### Privacy

Everything runs **in your browser** via WebAssembly — your secret, private key,
and claims are never uploaded to a server. You can also run it from the
[gizza CLI](/) or inside a gizza chat.

### Notes

- The payload and header must each be a **JSON object**. Standard registered
  claims include `iss`, `sub`, `aud`, `exp` (expiry, seconds since epoch), `nbf`,
  `iat`, and `jti` — but any keys are allowed.
- For HS* the secret is taken as raw UTF-8 bytes (the same way most JWT libraries
  treat a string secret).
- Verify the token with any standard JWT library using the **same algorithm** and
  the secret (HS*) or the matching **public** key (RS*/ES*). Need a key pair? See
  the RSA / ECDSA key-pair generator tools.

### FAQ

<details>
<summary>Can I add extra header fields like "kid"?</summary>

Yes — paste a JSON object into the header field (e.g. `{"kid":"2024-key-1"}`) and it's merged in. Two fields are managed for you: `alg` is always overwritten with the algorithm you selected, and `typ` defaults to `JWT` unless your header sets its own value.

</details>

<details>
<summary>What key format do RS256 and ES256 expect?</summary>

A PEM private key pasted into the secret field. RSA accepts both PKCS#8 (`BEGIN PRIVATE KEY`) and PKCS#1 (`BEGIN RSA PRIVATE KEY`); ECDSA requires PKCS#8 with a P-256 key for ES256 or P-384 for ES384. Verification is done elsewhere with the matching *public* key.

</details>

<details>
<summary>Does the tool add exp or iat claims automatically?</summary>

No — the payload is signed exactly as you wrote it. If you want an expiry, add `"exp"` yourself as seconds since the Unix epoch (e.g. `{"sub":"alice","exp":1767225600}`). That keeps the tool predictable for testing tokens with any claim combination.

</details>

<details>
<summary>Why does my token verify differently than in some other library?</summary>

Check three things: the algorithm must match exactly (HS256 vs HS512 tokens are incompatible), the HS* secret is taken as raw UTF-8 bytes (not base64-decoded — some libraries offer a "secret is base64" toggle), and ES* signatures use the JWS raw `r‖s` format, not DER.

</details>

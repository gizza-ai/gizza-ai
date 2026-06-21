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

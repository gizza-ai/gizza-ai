## About this tool

**JWT verify** decodes a **JSON Web Token** (JWT — the JWS compact
serialization) and verifies it two ways: it checks the **cryptographic
signature** against your key, and it validates the standard **claims**
(`exp`, `nbf`, `iss`, `aud`). The result is a clear report telling you whether
the token is `valid`, which checks passed, and — if it isn't — exactly why.

- **HMAC (shared secret):** `HS256`, `HS384`, `HS512` — verify with the same
  secret string used to sign the token.
- **RSA (public key):** `RS256`, `RS384`, `RS512` — RSASSA-PKCS#1 v1.5. Paste a
  PEM public key (`-----BEGIN PUBLIC KEY-----` SPKI, or
  `-----BEGIN RSA PUBLIC KEY-----` PKCS#1).
- **ECDSA (public key):** `ES256` (P-256), `ES384` (P-384) — JWS raw `r‖s`
  signatures. Paste a PEM PKCS#8 EC public key.

### What gets checked

- **Signature** — always verified against the key. A mismatch means the token
  was tampered with or signed with a different key.
- **`exp`** (expiry) and **`nbf`** (not-before) — compared against the current
  time, with an optional **leeway** to absorb small clock skew.
- **`iss`** (issuer) and **`aud`** (audience) — checked only when you supply an
  expected value. The audience may be a string or an array.

### Security

- Setting a **required algorithm** makes verification reject any token whose
  `alg` header doesn't match — the standard defense against
  algorithm-confusion / downgrade attacks. Tokens using the unsecured
  `alg: none` are always rejected.
- Everything runs **in your browser** via WebAssembly — your token and key are
  never uploaded to a server. You can also run it from the
  [gizza CLI](/) or inside a gizza chat.

### Notes

- Need to create tokens to test against? See the **JWT sign** tool. Need an RSA
  or EC key pair? See the key-pair generator tools.
- For HS* the secret is taken as raw UTF-8 bytes (the same way most JWT
  libraries treat a string secret).

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

## FAQ

<details>
<summary>What goes in the key field — a secret, a public key, or a private key?</summary>

It depends on the algorithm. For HS256/384/512, paste the shared secret string
(interpreted as raw UTF-8 bytes, as most JWT libraries do). For RS* and ES*,
paste the PEM **public** key — `-----BEGIN PUBLIC KEY-----` (SPKI) or
`-----BEGIN RSA PUBLIC KEY-----` (PKCS#1). Never paste a private key;
verification doesn't need it.

</details>

<details>
<summary>Do I need to fill in the required-algorithm field?</summary>

It's optional but recommended: when set, any token whose `alg` header differs
is rejected outright, which blocks algorithm-confusion and downgrade attacks.
Left empty, the token's own `alg` is used — except `alg: none`, which this
tool always refuses.

</details>

<details>
<summary>A token that just expired still fails — can I allow for clock skew?</summary>

Yes, that's the **leeway** field: the number of seconds of tolerance applied
to both the `exp` (expiry) and `nbf` (not-before) checks. It defaults to 0,
so even one second past expiry fails until you grant some leeway.

</details>

<details>
<summary>How are the issuer and audience claims matched?</summary>

Both are only checked when you supply an expected value. The issuer must equal
the token's `iss` exactly. The audience passes if the token's `aud` — whether
a single string or an array — contains your expected value.

</details>

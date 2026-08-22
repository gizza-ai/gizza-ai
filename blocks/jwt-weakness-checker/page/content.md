## About this tool

A JSON Web Token is only base64url-encoded, not encrypted, and the security of the whole
scheme rests on a handful of details that are easy to get wrong: the signing algorithm, the
strength of the HMAC secret, and the claims that bound the token's validity. This checker
reads a compact JWT and reports what an attacker would notice about it — no key required, and
the token is analysed entirely on your own device.

### What it checks

- **`alg: none`** — an unsecured token that carries no signature at all. Anyone can rewrite
  the claims and a verifier that honours the `none` algorithm will accept them.
- **Weak or guessable HMAC secrets** — for `HS256`/`HS384`/`HS512` tokens the signature is
  recomputed against a built-in list of well-known example, default and tutorial secrets. If
  one reproduces the signature, the secret is reported outright. Add your own candidates
  (deployment names, old passwords, environment strings) in the second field.
- **Expiry** — missing `exp`, an `exp` already in the past, and lifetimes longer than your
  chosen threshold.
- **Best-practice claims** — missing `iss`, `aud` and `iat`, and a `nbf` that has not arrived.
- **Header hygiene** — a missing `typ`, an unrecognized algorithm, and a `kid` header (which
  becomes an injection or path-traversal surface if it flows unvalidated into a key lookup).
- **Algorithm-confusion surface** — an informational note on asymmetric tokens, where a
  verifier that trusts the header's `alg` can be tricked into treating the public key as an
  HMAC secret.
- **Sensitive data in the payload** — claim names such as `password`, `ssn` or `api_key`,
  which are readable by anyone holding the token.
- **Oversized tokens** — anything past 4 KB, which starts to collide with header and cookie
  limits.

Every finding carries a severity (`info`, `low`, `medium`, `high`, `critical`), an explanation
and a concrete recommendation. The findings roll up into a 0–100 risk score and a level; any
critical finding — `alg: none` or a cracked secret — puts the token at `critical` on its own.

### Worked example

Audit the classic demo token, signed `HS256` with the secret `secret`:

**Input**

```
eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.XbPfbIHMI6arZ3Y922BhjWgQzWXcXNrz0ogtVhfEd2o
```

**Output** (abbreviated)

```json
{
  "algorithm": "HS256",
  "cracked_secret": "secret",
  "risk_score": 100,
  "risk_level": "critical",
  "findings": [
    {
      "id": "weak_secret",
      "severity": "critical",
      "title": "Weak HMAC secret — signature cracked",
      "detail": "The token's HMAC signature was reproduced with a common/guessable secret (6 chars)...",
      "recommendation": "Rotate to a high-entropy random secret of at least 32 bytes (256 bits)..."
    },
    {
      "id": "exp_missing",
      "severity": "high",
      "title": "No expiry (missing 'exp')",
      "...": "..."
    }
  ]
}
```

The signature was reproduced from a dictionary in milliseconds, and the token never expires —
two independent reasons to rotate the secret and reissue.

### Limits and edge cases

- **Signed tokens only.** Compact JWS with three segments, or a two-segment unsecured token.
  Encrypted JWE tokens (five segments) cannot be inspected offline and are rejected with an
  explanation.
- **The secret hunt is a dictionary attack, not a brute force.** It tries the built-in list of
  roughly fifty well-known secrets plus whatever you paste in — it does not enumerate character
  combinations, so a random 32-byte secret will never be "cracked" here. A clean result means
  "not obviously weak", not "provably strong".
- **HMAC only.** `RS*`, `PS*`, `ES*` and `EdDSA` tokens are audited for claims, header hygiene
  and algorithm-confusion surface, but their signatures use asymmetric keys and cannot be
  guessed from a wordlist.
- **This tool does not verify signatures against a key you hold** — that is a separate job; use
  the JWT verify tool for that.
- **Sensitive-claim detection is name-based.** It matches well-known claim names like
  `password` or `ssn`; a secret hidden under a custom claim name will not be spotted.
- **The clock comes from your browser.** Expiry findings are relative to your device's current
  time; use the leeway field if the issuer's clock is known to drift.
- **Nothing is uploaded.** The audit runs in WebAssembly in this page. Even so, treat any live
  production token you paste anywhere as compromised and rotate it.

## FAQ

<details>
<summary>Is my token sent to a server?</summary>

No. The audit is compiled to WebAssembly and runs inside this page, so the token, your
wordlist and the results stay on your device. That said, a live token pasted into any tool is
best treated as exposed — rotate it afterwards.

</details>

<details>
<summary>What does "signature cracked" actually mean?</summary>

For an HMAC token the signature is `HMAC(secret, header.payload)`. The checker recomputes that
value with each candidate secret and compares it to the signature on the token. A match proves
the candidate *is* the signing secret, which means anyone with the same list can mint valid
tokens for your system. Rotate to a random secret of at least 32 bytes and reissue.

</details>

<details>
<summary>My token came back clean. Is it secure?</summary>

It means none of these checks fired: the algorithm is a real one, the secret is not in the
dictionary, the expiry is present and reasonable, and the standard claims are set. It does not
prove the secret is strong, that your server pins the expected algorithm, or that the token is
being validated correctly on the other end. Treat a clean report as a floor, not a guarantee.

</details>

<details>
<summary>Why is a long expiry flagged when the token is otherwise fine?</summary>

A leaked token stays usable until it expires, so lifetime is the size of the blast radius.
Access tokens are usually minutes to hours; anything measured in months is worth a second look.
The threshold is yours to set — drag the slider, or set it to 0 to switch the check off.

</details>

<details>
<summary>How do I test a secret specific to my organisation?</summary>

Paste candidates into the extra-secrets field, one per line or comma-separated — old deployment
names, environment strings, previous passwords, anything a developer might have typed in a
hurry. They are tried after the built-in list, and a hit is reported the same way.

</details>

<details>
<summary>Can it check RS256 or ES256 tokens?</summary>

It audits their claims, header hygiene and algorithm-confusion surface, and flags anything
unusual. It cannot test their signatures: those are made with a private key, and no wordlist
can recover one. To confirm an asymmetric signature you need the matching public key and a
verification tool.

</details>

<details>
<summary>What is the "kid" warning about?</summary>

The `kid` header names which key should verify the token, and it is attacker-controlled. If a
server drops that value straight into a file path, SQL query or URL, the token becomes a
path-traversal, injection or SSRF vector. The finding is informational — it flags the surface,
not a confirmed bug. Look up `kid` against an allow-list rather than interpolating it.

</details>

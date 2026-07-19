## About this tool

**JWT Claims Diff** decodes two compact JSON Web Tokens locally and compares the
claims in their payloads. It shows which top-level claims were **added**,
**removed**, or **changed**, how many stayed the same, and whether the tokens are
claim-equivalent.

- **Payload diff by default**: `sub`, `role`, `scope`, `exp`, and other claims are
  compared at the top level.
- **Optional header diff**: include JOSE header changes such as `alg`, `typ`, or
  `kid` when you need to spot signing/configuration changes.
- **Time claim annotations**: numeric `exp`, `nbf`, and `iat` claims include a
  human-readable UTC timestamp; two `exp` claims also get a delta in seconds.
- **Structured JSON output**: use the report in scripts or copy it into a review.

No verification key is requested and signatures are **not** verified. This tool is
for offline inspection and comparison only; use a JWT verification tool when you
need to prove a token is authentic.

### Worked example

Comparing an old token with `role: "user"` and `scope: ["read"]` to a new token
with `role: "admin"`, `scope: ["read", "write"]`, and a new `plan` claim returns
an excerpt like:

```json
{
  "equal": false,
  "summary": { "added": 1, "removed": 0, "changed": 3, "unchanged": 3 },
  "payload": [
    { "claim": "role", "kind": "changed", "old": "user", "new": "admin" },
    { "claim": "scope", "kind": "changed", "old": ["read"], "new": ["read", "write"] },
    { "claim": "plan", "kind": "added", "new": "pro" }
  ]
}
```

### Limits & edge cases

- Claims are compared **top-level claim by top-level claim**. Nested objects and
  arrays are shown as a whole changed value rather than a recursive JSON Patch.
- Tokens are decoded but not verified. A forged token can still be decoded and
  diffed.
- Encrypted JWT/JWE values are not supported; the input must be a readable
  compact JWT with JSON header and payload parts.
- Very large custom claims produce a correspondingly large JSON report.

## FAQ

<details>
<summary>Does this verify the JWT signature?</summary>

No. It deliberately decodes without a verification key and compares the decoded
JSON. Use it to understand what changed between two tokens, not to decide whether
either token should be trusted.

</details>

<details>
<summary>Can it compare the token header as well as the payload?</summary>

Yes. Leave “Also diff the JOSE header” enabled to include header parameters such
as `alg`, `typ`, and `kid`. Turn it off when you only care about payload claims.

</details>

<details>
<summary>How are expiration changes reported?</summary>

When both payloads include numeric `exp` values, the report includes each expiry
as UTC plus `delta_seconds`, so you can quickly see whether the second token
expires earlier, later, or at the same time.

</details>

<details>
<summary>Does it recursively diff nested claim objects?</summary>

No. Top-level claims are the unit of comparison. If a nested object or array
changes, that whole claim is marked as changed and both values are shown.

</details>

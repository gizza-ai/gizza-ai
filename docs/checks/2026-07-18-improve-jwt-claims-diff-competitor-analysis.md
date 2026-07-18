# jwt-claims-diff — competitor analysis (2026-07-18)

Scope: compare two compact JWTs offline and report claim-level additions,
removals, and changes. This complements existing decode/verify/sign tools: it is
not a verifier and does not require a key.

Sources skimmed (paraphrased; no copy/branding reproduced):

- JWT.io debugger: decodes header/payload and validates signatures when a key is
  supplied, but does not present a two-token diff.
- Token.dev / JWT debug tools: decode, inspect registered claims, and sometimes
  verify, but comparison is manual.
- Generic JSON diff tools: compare decoded payloads well, but require manually
  extracting the payload and do not understand JWT time claims.
- API/security workflows in docs/blogs: common need is spotting role/scope/expiry
  changes between a before/after token during auth debugging.

## Table-stakes → decision

| Capability | In-model? | Decision |
|---|---|---|
| Decode two compact JWTs locally | yes | Built using the existing jwt-decode core, without signature verification. |
| Added / removed / changed claim classification | yes | Built for top-level payload claims, with structured JSON report. |
| Optionally compare JOSE header fields | yes | Built via `include_header` checkbox (default true). |
| Expiry / time claim readability | yes | Built for `exp`, `nbf`, and `iat`; includes expiry delta when both `exp` values are numeric. |
| Machine-readable output | yes | Built as pretty/minified JSON with `indent`. |
| Recursive JSON Patch-style nested diff | partial | Out of scope for this first tool; nested claim values are shown whole. |
| Signature verification | no | Out-of-scope by design; existing `jwt-verify` handles authenticity. |
| JWE encrypted token support | no | Out-of-model here; needs decryption keys and algorithms, not an offline diff of readable claims. |

## UX / controls

- Two multiline JWT fields: old/left and new/right.
- Checkbox for header comparison because many users only care about payload.
- Numeric indent field with 0 boundary for minified CLI/script output.
- Preset examples demonstrate a role/scope upgrade and payload-only comparison.

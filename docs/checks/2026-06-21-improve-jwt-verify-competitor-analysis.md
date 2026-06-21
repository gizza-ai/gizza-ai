# jwt-verify — competitor analysis (2026-06-21)

New tool built from the backlog: **jwt-verify** — verify a JSON Web Token's
signature and validate its `exp` / `nbf` / `iss` / `aud` claims. Complements the
existing **jwt-sign** tool (same algorithm matrix, public-key counterpart).

## Surfaces verified

- **Chat / LLM API** — `gizza-ai/jwt-verify` skill block; schema single-sourced
  from `descriptor()` with a drift-guard test (`schema_json_matches_authored_chat_schema`).
- **CLI** — `gizza tool jwt-verify token=… key=… [algorithm=… issuer=… audience=… leeway=… now=…]`.
- **Page** — `/tools/jwt-verify/` (in-browser wasm; token + key + algorithm +
  issuer + audience + leeway inputs; the clock comes from `Date.now()`).

## Top competitors surveyed

1. **jwt.io** (Auth0/Okta) — the reference decoder/verifier. Decodes header +
   payload, verifies HS/RS/ES/PS signatures, shows a verified/invalid badge.
2. **8gwifi.org JWT Debugger** — all algorithms incl. ES512/PS*, expiration
   check, claim inspection, timing-attack note, 100% client-side.
3. **devglan JWT Validator** — HS256/384/512, RS256/512, PS256; separate
   header-validation and claim-validation (exp/nbf/iss/aud) sections with a
   per-claim ✔ status; expected issuer/audience inputs.
4. **Authgear JWT & JWE Debugger** — decode/verify plus JWE encrypt/decrypt.
5. **jwt.is** — focused decoder with claim explanations.

## Feature diff (theirs → ours)

| Capability | Competitors | gizza jwt-verify |
| --- | --- | --- |
| Signature verify HS256/384/512 | yes | **yes** |
| Signature verify RS256/384/512 | yes | **yes** |
| Signature verify ES256/384 | most | **yes** |
| `exp` / `nbf` time checks | yes | **yes** (caller-supplied clock; deterministic core) |
| `iss` / `aud` claim checks | devglan, others | **yes** (aud may be string or array) |
| Per-check result breakdown | devglan (sections) | **yes** — structured `checks[]` with name/ok/detail |
| Clock-skew **leeway** | rarely exposed | **yes** — explicit `leeway` seconds on exp/nbf |
| **alg-confusion** defense (require alg) | best-practice, seldom in UI | **yes** — optional required `algorithm`; `alg:none` always rejected |
| Client-side / no upload | yes | **yes** — wasm in browser; CLI/chat run locally |
| Decode without verifying | yes | core `decode()` exposed (header/payload always returned) |

## In-model gaps closed

- Added a **structured per-check report** (`checks[]` = signature, exp, nbf, iss,
  aud) so the result says *exactly* why a token is invalid, matching/beating
  devglan's split sections.
- Added **leeway** (clock-skew tolerance) on `exp`/`nbf` — a real-world need most
  online validators omit.
- Added **required-algorithm** enforcement + unconditional `alg:none` rejection —
  the 2026 best-practice alg-confusion mitigation, surfaced as a first-class input.
- `aud` accepts both the string and array JWT forms.

## Out-of-model / deliberately omitted

- **PS256/384/512 (RSA-PSS)** and **ES512 (P-521)** — omitted to mirror jwt-sign,
  which omits them for the same reasons recorded in the build findings (P-521
  ECDSA is randomized-only / non-deterministic; PSS was not added to the signing
  side). Verification stays HS/RS(PKCS#1 v1.5)/ES256/384.
- **JWE encrypt/decrypt** (Authgear) — out of scope; this tool is JWS verification.
- **JWKS / remote key fetch by `kid`** — would require a network fetch and key-set
  parsing; not built. Users paste the secret or PEM public key directly.

No competitor copy, branding, or trademarks were used.

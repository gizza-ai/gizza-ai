# jwt-weakness-checker — competitor analysis (2026-08-22)

Scan run before implementing. Goal: an offline JWT security auditor that flags common
weaknesses (alg:none, weak/guessable HMAC secrets, missing/expired/over-long expiry, missing
best-practice claims) and returns a prioritized findings list + risk score. All analysis is
pure-Rust, runs locally, no token ever leaves the device.

## Competitors surveyed

1. **JWTAuditor** (jwtauditor.com) — client-side decode + analyze + brute-force; detects weak
   algorithms, expired tokens, missing claims; bruteforces HMAC secrets in-browser.
2. **SecurityWall JWT Analyzer** (securitywall.co/tools/jwt-analyzer) — decode/verify/audit
   locally; risk score 0–100; timeline analysis; weak-secret wordlist test (~104k entries,
   custom wordlists, smart domain guesses); flags none-alg, distant exp, missing iss/aud,
   sensitive-data exposure, kid confusion, token size > 4 KB, missing typ.
3. **Selqio JWT Security Analyzer** (selqio.com) — none-alg attacks, weak keys, missing claims,
   algorithm weakness, expiration validity, aud/iss mismatch, weak-secret brute-force; Trust
   Score; analysis-setting checkboxes; sample/clear/export.
4. **JWT Toolkit / jwtcracker.com** — bruteforce (multiple wordlist sources: ~1k AI, ~10k/~100k
   common, SecLists, custom), decoder, attacks, scanner, re-signer; HMAC only for cracking
   (HS256/384/512); progress metrics.
5. **Exploit-Forge JWT Security Checker** — none algorithm, weak secrets (dictionary attack,
   custom wordlist ≤ 2 MB), insecure claims; algorithm selection HS/RS/ES/none; one-click scan.

## Table-stakes → decision (in-model unless noted)

| Capability | Decision |
|---|---|
| Detect `alg: none` (+ case variants None/NONE) | **in** — critical finding |
| Detect weak-secret via wordlist brute-force (HMAC) | **in** — built-in common-secret list + user `wordlist` param; reports the cracked secret |
| Missing `exp` (no expiry) | **in** — high finding |
| Expired token (`exp` in the past) | **in** — high finding |
| Over-long lifetime (`exp` far in future) | **in** — medium; `max_exp_days` threshold (default 30) |
| `nbf` not yet valid / missing `iat` | **in** — low/info |
| Missing `iss` / `aud` best-practice claims | **in** — low findings |
| Missing `typ` header | **in** — info |
| `kid` header present (injection/path-traversal surface) | **in** — info |
| Algorithm-confusion risk (RS/ES keys usable as HMAC) note | **in** — info when asymmetric alg present |
| Sensitive data in payload (password/secret/ssn/… claim keys) | **in** — medium; simple key-name match |
| Oversized token (> 4 KB) | **in** — low |
| Risk score 0–100 + level (low/medium/high/critical) | **in** — weighted by severity |
| Custom secret wordlist | **in** — `wordlist` param (newline/comma separated) |
| Sample-token / clear / export report buttons | **in (partial)** — `[[example]]` preset chip prefills a sample; export = copy/download of the text output the page already provides |
| Full 100k+ SecLists brute-force, 15–25k keys/s metrics | **out-of-model** — embedding a 100k wordlist bloats the wasm; ship a curated common-secret list (~50) + user-supplied wordlist instead. Documented on page. |
| PDF/Markdown formatted report export | **out-of-model** — page output is plain text; users copy/download it |
| Re-signing / token editing / attack payload generation | **out-of-model** — offensive token forging is out of scope for a defensive checker (jwt-sign already covers legitimate signing) |
| Signature verification against a provided key | **out-of-model here** — already covered by `blocks/jwt-verify`; this tool audits weaknesses, links users to jwt-verify for key verification |

## Non-duplicate rationale

- `jwt-decode` decodes header/payload + validates exp/nbf/iat but does **no** security scoring,
  no weak-secret cracking, no best-practice/none-alg auditing.
- `jwt-verify` verifies a signature against a **supplied** key; it doesn't hunt for weak secrets
  or produce a findings/risk report.
- `jwt-sign` / `jwt-claims-diff` are unrelated (signing, diffing).

This tool's distinct capability = **audit + weak-secret brute-force + risk report**, none of
which any existing block provides.

No competitor copy, branding, or wordlists were reproduced; the built-in secret list is a small
original set of well-known example/default secrets.

# http-header-analyzer — competitor analysis (2026-06-23)

## Tool

`blocks/http-header-analyzer` — paste a block of HTTP **response** headers; get a
plain-English explanation of each header (caching, compression, content, cookies,
CORS, server hints), a list of **missing recommended security headers** with fixes,
**value-quality warnings** for present security headers, and an overall **A+ → F
security grade**. Pure-Rust → runs on all three surfaces (chat block, page, CLI).

## Top competitors surveyed

1. **securityheaders.com** (Scott Helme) — the market-leading scanner. Grades a
   site **A+ → F**. Checks: Content-Security-Policy, Strict-Transport-Security,
   X-Frame-Options, X-Content-Type-Options, Referrer-Policy, Permissions-Policy,
   X-Permitted-Cross-Domain-Policies. Crucially it grades the **quality of present
   header values** (CSP `unsafe-inline`/bypassable allowlist, HSTS too short to
   preload, weak Referrer-Policy, deprecated directives), not just presence.
2. **Mozilla HTTP Observatory (MDN)** — in-depth, score-based assessment of headers
   + other config; actionable feedback. URL-fetch based.
3. **HackerTarget HTTP Header Check** — raw header dump + framing of what the
   headers leak (server software, framework, infra). URL-fetch based.
4. **APIVoid / AquilaX / DNS Robot security-headers checkers** — URL-fetch scanners
   detecting CSP/HSTS/X-Frame-Options/Referrer-Policy with A–F grades and
   recommendations.
5. **OWASP Secure Headers Project + HTTP Headers Cheat Sheet** — the canonical
   recommended set and values; recommends removing `Server`, `X-Powered-By`,
   `X-AspNet-Version`, `X-AspNetMvc-Version`, and treats `X-XSS-Protection` as
   deprecated (set `0` / remove, use CSP).

## Gap diff and what was closed (all in-model, pure-Rust)

| Capability | Before | Competitor has it | Action |
|---|---|---|---|
| Explain each header in plain English | yes | partial (most only grade security headers) | kept — a differentiator |
| Caching / compression / CORS / cookie analysis | yes | no (security-only scanners) | kept — broader than competitors |
| List missing recommended security headers + fix | yes | yes | kept |
| **Overall A+ → F security grade** | **no** | yes (securityheaders, APIVoid, DNS Robot) | **added** `security_grade` |
| **Grade VALUES of present headers** (CSP unsafe-inline/unsafe-eval, short HSTS, weak Referrer-Policy, obsolete X-Frame-Options) | **no** | yes (securityheaders) | **added** value-quality findings + grade penalty |
| **Flag deprecated `X-XSS-Protection`** | **no** | yes (OWASP) | **added** |
| **Flag more fingerprinting headers** (`X-AspNet-Version`, `X-AspNetMvc-Version`) | partial (Server/X-Powered-By only) | yes (OWASP) | **added** |

## Out-of-model features (NOT built — by design)

- **URL fetching / live scanning** — every URL-based competitor fetches the target
  itself. This tool is a **paste-in** analyzer by design (privacy: nothing is
  uploaded from the page; the chat/CLI surfaces have a separate SSRF-guarded
  fetcher). Live fetch would be a separate network tool.
- **TLS / certificate / cookie-jar / DNS checks** — out of scope for a header
  analyzer.
- **Trademark/branding** — no competitor copy, branding, or grading rubric was
  copied; the grade is a simple presence-minus-weakness heuristic of our own.

## Verification (this run)

- `cargo test --workspace`: 23 core unit tests + 1 block drift-guard schema test — all pass.
- `wafer build`: chat `block.wasm` validates/instantiates (349.2 KiB).
- CLI: `gizza tool http-header-analyzer headers=…` returns JSON with
  `security_grade`, `missing_security_headers`, value-quality `findings`.
- Page: Playwright `tool-page-http-header-analyzer.spec.ts` — 2 specs pass
  (grade + header explanations + Set-Cookie hardening gaps).

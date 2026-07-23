# har-redact — competitor analysis (2026-07-23)

Tool: **har-redact** — replace sensitive VALUES in a HAR (HTTP Archive) capture with a
placeholder (cookies, auth/API-key headers, sensitive query-string params, request/response
bodies) so a capture is safe to attach to a bug report while keeping its full structure for
debugging. Distinct from the existing `har-body-stripper` (which DELETES body fields to shrink a
file and deliberately leaves cookies/headers untouched — see its own copy, "that is a redaction
tool's job") and from `json-redact` (generic JSON secret masking, not HAR-header/cookie/queryString
aware).

All notes below are **paraphrased** from public tool pages/READMEs — no competitor copy, branding,
or trademarks reproduced.

## Competitors scanned

1. **Google har-sanitizer** (github.com/google/har-sanitizer) — client-side web tool + Flask REST
   API. Redacts values matching a default sensitive **word list**, plus user-supplied words.
   Discovery endpoints enumerate the cookie/header/param/mimetype names present in a capture so the
   user can pick. Toggles: `all_cookies`, `all_headers`, `all_params`, `all_content_mimetypes`
   (redact every one indiscriminately), `wordlist` (append custom terms), `content_list` (mimetypes
   to redact). Default sensitive terms observed: state, password, email, code, code_verifier,
   client_secret, client_id, token, access_token, authenticity_token, id_token, appID, challenge,
   facetID, assertion, Authorization, auth, x-client-data, SAMLRequest, SAMLResponse.
2. **OpenBrowserTools HAR File Sanitizer** — HAR viewer + sanitizer. Redacts, as separate
   categories: cookies, auth headers, bearer tokens, API keys, session IDs, query values, request
   bodies, response bodies. Browser-local.
3. **PunkFix HAR Sanitizer** — toggles for Cookies / Authentication / Passwords; 100% in-browser,
   Upload → Process → Download; stated max 20 MB.
4. **WSO2 HAR Sanitizer** (support reference) — can hash OR remove sensitive values from traces
   (session cookies, auth headers, …).
5. **Cloudflare HAR Sanitizer** (har-sanitizer.pages.dev) — client-side removal of sensitive data.

## Table-stakes params / UX (tagged in-model / out-of-model)

| Capability | Competitors | Decision |
| --- | --- | --- |
| Redact cookie values (request/response cookie arrays + Cookie/Set-Cookie headers) | all | **in-model** — `cookies` toggle, default on |
| Redact Authorization + API-key/token headers | Google, OBT, PunkFix | **in-model** — `auth_headers` toggle, default on (built-in header list) |
| Custom extra header names to redact | Google (`all_headers`/wordlist) | **in-model** — `extra_headers` (comma list) |
| Redact sensitive query-string param VALUES (in queryString[] and the URL) | Google, OBT | **in-model** — `query_params` toggle, default on (built-in word list) |
| Custom sensitive param names | Google (`wordlist`) | **in-model** — `sensitive_params` (comma list, merged with built-ins) |
| Redact request / response bodies | OBT | **in-model** — `bodies` enum none/request/response/both, default `response` |
| Customizable placeholder text | (varies) | **in-model** — `placeholder`, default `[REDACTED]` |
| Dry-run report of what would be redacted | Google discovery endpoints (partial) | **in-model** — `output=summary` (per-category counts) |
| Pretty vs compact output | (implicit) | **in-model** — `pretty` toggle, default off |
| Browser-local, nothing uploaded | all | **in-model** — gizza is wasm, browser-local by construction |
| Server-side REST API / batch endpoint | Google (Flask) | **out-of-model** — needs a backend; gizza is browser-local |
| Interactive per-field discovery pick-list UI (enumerate names, tick each) | Google, OBT viewer | **considered, rejected for v1** — stateful multi-step SPA; the single-shot page + `output=summary` covers "what would be redacted" without a slug-specific UI |
| Hash sensitive values instead of removing them | WSO2 | **considered, rejected for v1** — a hash still leaks value length/equality; "safe sharing" wants values gone. Kept the placeholder unambiguous; can revisit as a `mode` param later |

## Worked example (used on the page + Playwright)

Input: a HAR with a login request carrying a `Cookie` header + a `sessionid` cookie, an
`Authorization: Bearer …` header, a `?token=…` query param, and a JSON response body. With the
defaults (cookies + auth_headers + query_params on, bodies=response) the cookie/header/query values
and the response body text become `[REDACTED]`; URLs, methods, header names, status codes, timings,
and sizes are untouched, so the capture still opens in any HAR viewer.

## Decisions

- Method = value SUBSTITUTION (keep structure), NOT field deletion — this is the clean line vs
  `har-body-stripper`. Documented on the page and in both descriptors so the two tools cross-refer.
- Built-in sensitive-param word list (exact case-insensitive name match) derived from the union of
  the scanned tools; `sensitive_params` appends to it. Built-in auth/API-key header list similarly;
  `extra_headers` appends.
- Default `bodies=response` matches the backlog description ("cookies, auth headers, and response
  bodies") — request bodies (form passwords) are opt-in via `bodies=request`/`both`.
- Max 10,000 entries per run (family norm, matches har-body-stripper).
</content>
</invoke>

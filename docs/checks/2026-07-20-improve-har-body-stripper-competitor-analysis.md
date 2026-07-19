# har-body-stripper — competitor analysis (2026-07-20)

Scan done BEFORE implementation (create-next-tool step: competitor scan). Paraphrased notes only —
no competitor copy, branding, or trademarks reproduced in tool copy.

Scope note: this tool is the BODY stripper (shrink + de-sensitize by removing request/response
payloads). Cookie / auth-header / token redaction is deliberately out of scope — the backlog has a
separate `har-redact` tool for that, and `har-secret-scanner` for detection. The page copy
cross-references the boundary instead of silently overlapping.

## Competitors examined

1. **google/har-sanitizer** (github.com/google/har-sanitizer) — Python lib + web UI. Redacts
   passwords, cookies, headers, URL-query/POST/form parameters, and *embedded content by
   mimeType*: a default mimeType scrub list plus a user-supplied `content_list`, and a boolean
   `all_content_mimetypes` to wipe every body. Preview-before-commit flow, then export the
   scrubbed HAR.
2. **AbregaInc/har-cleaner** (github.com/AbregaInc/har-cleaner) — TS library behind a Jira
   support-portal cleaner. Separate booleans per surface: scrub all request headers / cookies /
   query params / POST params / response headers / **all body contents**, plus
   scrub-specific-mimeTypes with a default list. All-vs-specific flips deny-list to allow-list.
   Defaults derive from Cloudflare's word lists.
3. **frontegg/harmor (HARmor)** (github.com/frontegg/harmor) — npx CLI, interactive questionnaire
   or JSON template. Scrubs cookies, passwords, auth headers, query params; removes JSON body
   *keys*; URL-scoped rules; strips JWT signatures; can password-protect output.
4. **Cloudflare HAR Sanitizer** (har-sanitizer.pages.dev, blog.cloudflare.com) — client-side-only
   processing (privacy pitch), strips session cookies/JWTs by default, "just enough" mode that
   neuters an Access JWT (drops the signature) instead of deleting it.

(openbrowsertools.com HAR viewer/sanitizer was in the results but 403s the fetcher — replaced by
HARmor per the scan rule.)

## Table-stakes → decision

| Capability (competitor) | Tag | Where it landed |
|---|---|---|
| Remove request bodies and response bodies independently (har-cleaner's separate POST-params vs body-contents toggles) | in-model | `strip` enum: `both` / `request` / `response`, default `both` |
| MimeType-scoped body scrubbing (google default+custom mime lists; har-cleaner scrubSpecificMimeTypes) | in-model | `only_mime` comma-separated case-insensitive mimeType substrings; empty = every body |
| Wipe ALL bodies in one go (google `all_content_mimetypes`; har-cleaner scrubAllBodyContents) | in-model | the defaults (`strip=both`, `only_mime` empty) do exactly this |
| Client-side / nothing-uploaded processing (Cloudflare pitch; all four run locally) | in-model | pure Rust→wasm; page copy states it runs in the browser |
| Preview before committing (google UI) | in-model | `output=summary` dry-run report (counts + bytes + before/after size) vs `output=har` |
| Shrink metric / size feedback (the tool's own raison d'être; support portals cap HAR sizes) | in-model | summary reports input → output bytes and % smaller; compact JSON default (`pretty=false`) is itself a shrink lever (DevTools exports are pretty-printed) |
| Keep the file analyzable after scrubbing (all competitors keep entry metadata) | in-model | `content.size`, `mimeType`, `compression`, `bodySize`, timings, headers all survive; only `text`/`params`/`encoding` payloads go |
| WebSocket payloads (Chrome `_webSocketMessages` carry tokens; sanitizers that ignore them leak) | in-model | ws `data` payloads stripped by direction (`send`→request side, `receive`→response side); frame type/opcode/time kept |
| Size-threshold stripping (keep small JSON, drop big blobs — support-workflow ask) | in-model | `min_bytes` integer, default 0 = all bodies |
| Cookie / auth-header / query-param / token redaction (all four) | out-of-scope by design | separate backlog tool `har-redact`; stated on the page + FAQ |
| JSON body key-level redaction (HARmor removes body keys) | out-of-scope | that's redaction, not stripping — `har-redact` / existing `json-redact` territory |
| Secret detection heuristics (Cloudflare JWT logic) | out-of-scope | backlog `har-secret-scanner` |
| JWT signature neutering (Cloudflare "just enough") | OUT-OF-MODEL here | token-aware parsing belongs to the redaction tool, not the body stripper |
| Password-encrypted output archive (HARmor) | OUT-OF-MODEL | out of scope for a stateless browser text tool |
| URL-scoped rules (HARmor per-URL sanitization) | OUT-OF-MODEL (deliberate omit) | niche for a stripper; `only_mime` + `min_bytes` cover the practical cases; documented as a limit |
| File upload + download round-trip (competitor UIs) | partial | page is paste-a-HAR (consistent with har-validator / har-request-extract); `format="text"` pages get the generic Download link for the output |

## UX control patterns adopted

- `strip` and `output` as `<select>`s with friendly `[input.labels]`.
- `har` as a multiline textarea with a realistic HAR placeholder (matches sibling HAR tools).
- `pretty` as a checkbox (default off = compact = smallest output).
- `[[example]]` chips: full strip (defaults), binary-only mime filter (`image/,font/,video/`),
  dry-run summary — mirroring competitors' "all bodies", "by mimeType", and "preview" flows.
- Cap: 10,000 entries per run (stated on the page; boundary-tested at and one over).

# safelink-decoder — competitor analysis (2026-06-20)

Seventeenth `/create-next-tool` backlog pick. Pure-Rust (percent-encoding) text
tool, all 3 surfaces. The picker hinted "network" but it's PURE string decoding
(no fetch — which is the whole safety point). Research via `WebSearch`,
paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| Link Grabber | SafeLinks + Proofpoint, batch of many at once | capabilities |
| Spambrella / Vircom | Proofpoint URLDefense reverse | capabilities |
| VFIR.io | Proofpoint v2 + v3 and Microsoft SafeLinks | capabilities |
| SiteSpecter | gateway rewrites + percent + HTML entities; optional shortener resolution | capabilities |

## Gap diff vs our tool
Our tool: unwraps Outlook SafeLinks, Proofpoint URLDefense v2 (`-`/`_` restore) and
v3 (`/v3/__…__;`), Google redirects, and any generic `?url=/q=/u=/target=/redirect=`
redirector; follows NESTED wrappers (SafeLink→Google→…) up to 8 levels; batch via
per_line; unknown links pass through unchanged. Matches the core competitor set.

**In-model gaps considered, deferred (fit the model; minor):**
- **HTML-entity decoding** of the wrapper (e.g. `&amp;` in a pasted-from-HTML
  link) before parsing — a small pre-pass.
- **More vendors** (Mimecast `protect-*.mimecast.com/s/…`, Barracuda
  `linkprotect.cudasvc.com`) — add patterns as needed.

**Out-of-model (intentional):** **URL-shortener resolution** (bit.ly → final) —
that requires fetching the link, which defeats the safety purpose (inspect a
suspicious destination WITHOUT visiting it). We deliberately never fetch.

## Tested
unit (7: Outlook SafeLink, Google redirect, Proofpoint v2 (-/_ restore), nested
SafeLink-over-Google, plain-URL passthrough, per_line batch, empty error) +
drift-guard · wafer fixtures (1) · `wafer build` · wasm-pack web · generator ·
CLI (unwraps a SafeLink) · Playwright page + query deep-link (2 tests).

> Original work only — no competitor copy, branding, or trademarks copied.

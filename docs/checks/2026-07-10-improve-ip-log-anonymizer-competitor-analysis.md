# ip-log-anonymizer — competitor scan (2026-07-10)

Scan of the "IP anonymization / log IP masking / log redaction" space, run before
implementing. All findings paraphrased from public docs and tool pages — no competitor
copy, branding, or trademarks reproduced.

## Competitors skimmed (top real tools)

1. **Matomo IP anonymization** — server-side setting that zeros the trailing bytes of a
   visitor's IP before storage. Configurable strength: mask the last 1, 2 or 3 IPv4 bytes
   (and the equivalent IPv6 bytes). Two masked bytes is the commonly recommended default;
   the truncated IP still allows coarse country/region geolocation.
2. **Google Analytics IP anonymization** (the legacy `_anonymizeIp` / `anonymize_ip` flag,
   later always-on in GA4) — zeros the last octet of an IPv4 address and the last 80 bits
   of an IPv6 address *before* it is written to disk or used for geo lookup. Fixed
   granularity (1 IPv4 octet / IPv6 /48), no per-property tuning.
3. **Cloudflare / CDN log pseudonymization** — masks or hashes the client IP in exported
   logs; the IPv6 default keeps the /48 prefix. Hashing keeps per-visitor uniqueness for
   analytics without storing the raw address.
4. **Log-redaction / scrubbing libraries** (e.g. logredactor-style filters, `log-anonymizer`
   CLIs, SIEM masking rules) — regex-match IPs (and other PII) in log lines and replace
   with a fixed placeholder such as `[IP]` / `x.x.x.x`, or with a salted hash. Operate
   in-place so the rest of the line (path, status, timestamp, port) survives.
5. **General "anonymize IP address" web utilities** — paste-a-line tools that truncate a
   single IP to a chosen mask length; mostly single-address, not whole-log, and often
   without IPv6 or private-range handling.

Sources: matomo.org (Privacy → Anonymize data / Anonymize IP addresses),
developers.google.com/analytics (IP anonymization / anonymize_ip),
developers.cloudflare.com (Logs — fields / pseudonymization), assorted log-redaction
library READMEs and "anonymize IP" web utilities.

## Table-stakes features (tagged in-model / out-of-model)

| Feature | Tag | Decision |
| --- | --- | --- |
| Truncate/mask trailing IPv4 octets (GA 1-octet, Matomo 2-octet) | in-model | **built** — `mode=mask`, `ipv4_octets` 0–4, default 1 |
| Mask trailing IPv6 hextets, keep the /48 by default | in-model | **built** — `ipv6_groups` 0–8, default 5 (last 80 bits zeroed) |
| Salted, deterministic hash so equal IPs collapse to equal tokens | in-model | **built** — `mode=hash`, `salt`, `hash_length` 4–64, SHA-256 |
| Fixed-placeholder redaction (`[IP]`, `x.x.x.x`) | in-model | **built** — `mode=redact`, `replacement` token |
| Whole-log, in-place rewrite; ports/paths/timestamps preserved | in-model | **built** — regex-match + std IP-parser validation, surrounds untouched |
| Skip private / internal ranges (RFC1918, loopback, link-local, ULA) | in-model | **built** — `skip_private` boolean |
| IPv6 + IPv4-mapped IPv6 handled once (no double replace) | in-model | **built** — v6 resolved before v4; overlaps suppressed |
| Reject non-IP lookalikes (version strings, `999.1.1.1`) | in-model | **built** — parse-validate every match before replacing |
| Runs locally / no upload (privacy) | in-model | **built** — WebAssembly, browser-only |
| Geo-IP lookup on the masked address | out-of-model | **not built** — needs a geo DB; the tool only anonymizes, geolocation is a downstream concern |
| Anonymize other PII in the log (emails, user IDs, cookies) | out-of-model | **not built** — separate concern; see `csv-pii-redactor` / a text-redaction tool |
| Server-side / streaming ingestion at write time (like GA/Matomo) | out-of-model | **not built** — gizza is a browser/CLI batch tool, not a log pipeline |
| Format-preserving encryption of IPs (reversible with a key) | out-of-model | **not built** — hashing is intentionally one-way; reversible mapping needs key management |
| Per-line schema parsing (Apache/Nginx/JSON field extraction) | out-of-model | **not built** — in-place text rewrite is format-agnostic and covers every log shape |

## Descriptor decisions

Params: `text` (required, multiline), `mode` (enum mask/hash/redact, default mask),
`ipv4_octets` (0–4, default 1 = GA style), `ipv6_groups` (0–8, default 5 = keep /48),
`salt` (hash mode, default empty but strongly recommended), `hash_length` (4–64, default
12), `replacement` (redact token, default `[IP]`), `skip_private` (boolean, default false).

Defaults deliberately mirror the two reference tools: GA's single-octet IPv4 truncation and
its IPv6 /48, so the out-of-the-box result matches what analysts already expect; Matomo's
two-octet setting is a one-field change. Hash uses a salt because IPv4 space is small enough
(~4.3B) to brute-force an unsalted digest — this is called out in the copy and FAQ. The
amount of clamping (octets ≤4, hextets ≤8, hash 4–64) is enforced in-core so the page, CLI,
and chat surfaces behave identically. Output is the whole log with only the addresses
rewritten — byte-for-byte identical elsewhere.

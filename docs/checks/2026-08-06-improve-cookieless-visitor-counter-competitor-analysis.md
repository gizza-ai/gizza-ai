# cookieless-visitor-counter — competitor analysis (2026-08-06)

Scan run **before** implementation, per `/create-next-tool` step 4. All findings are
**paraphrased**; no competitor copy, branding, or trademark is reproduced. Out-of-model
features are listed, never built.

Backlog row:
`cookieless-visitor-counter | Counts unique visitors from a log using the daily-salted-hash
method, storing no cookies or PII. | pure`

## Dup check (blocking, done first)

`ls blocks/ | grep -iE "visitor|log|counter|analytic|privacy|hash|unique|traffic|session|anonym"`
plus a repo-wide grep for `unique visitor` / `cookieless` / `daily salt` (zero hits) and a read of
`docs/tool-skiplist.txt` (no entry pointing at this slug). Nearest neighbours and why each is
**not** this tool:

| Existing block | What it does | Why not a dup |
| --- | --- | --- |
| `log-analyzer` | Severity/error aggregation, top errors, volume timeline | Aggregates *messages*, has no visitor-identity concept and never dedupes by actor |
| `ip-log-anonymizer` | Rewrites IPs in place (mask/hash/redact), returns the anonymized log | A redaction transform — emits a log, not a count; no per-period dedup, no salt rotation |
| `bot-traffic-filter` | Classifies each hit bot vs human by user-agent | Classification + filtering; every hit stays a hit, nothing is deduped into visitors |
| `find-unique-lines` | Line-level dedup of arbitrary text | Dedupes whole *lines*; two requests from one visitor are different lines |
| `data-anonymizer`, `hash-text`, `sha256-hash` | Generic PII scrubbing / hashing primitives | Primitives, not a visitor-counting engine |

The distinct capability here is **identity-deduplicated counting under a rotating salt**:
`hash(salt ‖ period ‖ IP ‖ UA)` → distinct-count per period. No existing block does this.
**Verdict: viable, build it.**

## Competitors reviewed

1. **Plausible Analytics** (privacy-first hosted analytics) — the reference implementation of
   the method. Identifies a visitor as a hash over a per-site daily-rotated random salt plus
   the domain, the IP and the user-agent. The salt is regenerated every 24 h and the previous
   one is discarded, so a visitor ID is deliberately un-linkable across days; raw IP and UA are
   never persisted. Reports daily "unique visitors" alongside total pageviews.
2. **Fathom Analytics** (privacy-first hosted analytics) — same family: SHA-256 over IP,
   user-agent, site hostname and a site-scoped salt, with the raw IP discarded immediately
   after hashing. Confirms SHA-256 and the "hostname in the identity material" variant as the
   table-stakes construction.
3. **GoAccess** (open-source access-log analyzer, CLI/HTML) — the log-side reference. Treats
   *same IP + same date + same user-agent* as one unique visitor, which is exactly the
   `ip_ua` identity mode with a daily bucket, minus the hashing. Ships a crawler-exclusion
   switch and per-day visitor/pageview tables. Its own issue tracker shows users repeatedly
   confused by the IP+UA+date definition — a strong signal that the tool must **state the
   identity rule on the page**, not bury it.
4. **AWStats** (open-source log analyzer) — the older convention: a unique visitor is one
   **host/IP** per reporting period, user-agent ignored. Explicitly documents the shared-NAT
   caveat (an office behind one IP counts once). Justifies shipping an **IP-only identity
   mode** and documenting the same caveat honestly.
5. **Matomo / GA-style IP anonymization** (the surrounding privacy convention) — truncate the
   IP (IPv4 → /24, IPv6 → /48) *before* it is used for identification. Justifies a third
   identity mode that hashes the truncated network rather than the full address.

## Table-stakes → decision

| # | Table stake (from the scan) | Decision | Where it lands |
| --- | --- | --- | --- |
| 1 | SHA-256 salted hash as the visitor ID | **in-model — built** | `sha2 0.10` (proven wasm-safe, already used by `ip-log-anonymizer`) |
| 2 | Salt that rotates per period so IDs can't be linked across days | **in-model — built** | The period key is mixed **into** the hash input, so an ID is structurally un-linkable across periods — the same guarantee as a server-rotated daily salt, but deterministic and reproducible offline |
| 3 | User-supplied secret salt | **in-model — built** | `salt` param; blank uses a fixed built-in constant so results are reproducible |
| 4 | IP + user-agent identity (Plausible/GoAccess) | **in-model — built** | `identity = ip_ua` (default) |
| 5 | IP-only identity (AWStats) | **in-model — built** | `identity = ip` |
| 6 | IP truncation before identification (Matomo/GA) | **in-model — built** | `identity = network_ua` (IPv4 → /24, IPv6 → /48) |
| 7 | Per-day buckets + unique visitors AND pageviews per bucket | **in-model — built** | `period = day` default; report/table/json/csv all carry visitors + pageviews + views-per-visitor |
| 8 | Other bucket granularities (hourly/monthly/whole-file) | **in-model — built** | `period = hour \| day \| month \| total` |
| 9 | Crawler exclusion switch (GoAccess `--ignore-crawlers`) | **in-model — built** | `exclude_bots` boolean, default **true** (curated token list + `bot`/`crawl`/`spider`/`slurp` heuristic) |
| 10 | Apache/nginx Combined + Common log formats | **in-model — built** | `format = combined \| common`, plus `auto` sniffing |
| 11 | JSON/NDJSON structured logs (modern nginx/CDN) | **in-model — built** | `format = json`, flexible key aliases |
| 12 | CSV exports from a CDN/analytics dashboard | **in-model — built** | `format = csv`, header-aliased columns |
| 13 | Machine-readable output for piping | **in-model — built** | `output = json \| csv` |
| 14 | Show the pseudonymous IDs themselves (auditability) | **in-model — built** | `output = ids` — per-request period + visitor ID, the "prove no PII survives" view |
| 15 | Preset/example one-click inputs | **in-model — built** | three `[[example]]` chips |
| 16 | The sum-of-daily-uniques ≠ total-uniques nuance | **in-model — built** | Report states both, and the page explains why they differ |
| 17 | Bounce rate / session duration / entry-exit pages | **out-of-model — listed, not built** | Needs session reconstruction + referrer semantics; belongs to a session-analytics tool, not a visitor counter |
| 18 | Geo/country and device/browser breakdowns | **out-of-model — listed, not built** | Needs a bundled GeoIP database and a full UA-parsing table — large data assets, out of scope for a counting tool |
| 19 | Live/real-time dashboards, historical trend storage | **out-of-model — listed, not built** | Needs a server, an account and persistence; gizza tools are browser-local and stateless |
| 20 | Reverse-DNS / IP-range crawler *verification* | **out-of-model — listed, not built** | Needs live DNS; the tool classifies by declared user-agent only and says so |
| 21 | Referrer/campaign attribution | **out-of-model — listed, not built** | A separate analytics dimension, not visitor identity |
| 22 | Top-pages / top-errors aggregation | **considered, rejected** | Already `blocks/log-analyzer`'s job — duplicating it here would fragment log aggregation across two tools |

## UX control patterns adopted

- Fixed choices are `Param::enumv` → real `<select>`s, with `[input.labels]` giving friendly
  option text (e.g. "IP + user-agent (Plausible/GoAccess)") while values stay canonical.
- `multiline = true` on the log field so pasted newlines survive.
- Placeholders on every text/number field show a **real** CLF line, not prose.
- Three `[[example]]` chips (daily report · hourly JSON · pseudonymous IDs) — competitors all
  ship presets/demo data, so the chips are the declarative answer.
- Limits (max lines, max emitted rows, undated-line handling, shared-NAT caveat) are stated on
  the page rather than discovered through an error.

## Honest limitations recorded on the page

- Shared NAT/CGNAT collapses many people behind one IP into one visitor; a rotating mobile IP
  or a UA change splits one person into several. This is inherent to logs — every competitor
  in this class has it, and the page says so.
- IDs are intentionally un-linkable across periods, so **daily uniques do not sum** to a
  monthly unique count; re-run with `period = month` for that number.
- Bot exclusion is by declared user-agent only — no reverse DNS or IP-range verification.
- The tool never stores anything: it is a pure function in WebAssembly, in the browser tab.

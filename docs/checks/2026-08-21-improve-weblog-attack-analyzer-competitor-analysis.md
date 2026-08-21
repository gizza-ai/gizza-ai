# weblog-attack-analyzer — competitor analysis (2026-08-21)

Scan run before completion. This is a paraphrased comparison of common public/open-source web-log security triage tools; no competitor wording, branding, or trademarks is reused in tool copy.

## Tools reviewed

| # | Tool shape | Relevant behaviour |
|---|------------|--------------------|
| 1 | GoAccess-style access-log analyzer | Parses Apache/Nginx common and combined logs, summarizes visitors/statuses/requests, and highlights high-volume sources. |
| 2 | Fail2ban-style web jail/filter rules | Detects repeated 404/401/403 and known bad URL probes, then emits offending IPs suitable for blocking. |
| 3 | WAF/IDS log triage patterns (ModSecurity/OWASP CRS style) | Uses SQLi/XSS/RCE/traversal signatures, severity classes, encoded-payload handling, and category-oriented reporting. |
| 4 | IIS Log Parser-style workflows | Parses IIS W3C `#Fields:` logs and lets analysts query suspicious URLs, status codes, and client IPs. |

## Table-stakes matrix

| Capability | Seen in | Verdict | Where it landed |
|---|---|---|---|
| Apache/Nginx common and combined parsing | 1, 2, 3 | in-model | CLF/combined regex parser with request target, status, timestamp, source IP, and user agent. |
| IIS W3C extended parsing | 4 | in-model | `#Fields:` header support for `date`, `time`, `c-ip`, `cs-method`, `cs-uri-stem`, `cs-uri-query`, `cs(User-Agent)`, and `sc-status`. |
| SQLi/XSS/traversal/RCE/file-inclusion signatures | 2, 3 | in-model | Curated category tables with severity mapping and matched signature names. |
| Percent-decoded payload checks | 3 | in-model | `decode=true` checks raw, once-decoded, and twice-decoded targets. |
| Scanner user-agent detection | 2, 3 | in-model | sqlmap, nikto, nuclei, wpscan, masscan, nmap, and related scanner UAs. |
| Sensitive-path probes | 2, 3 | in-model | `.env`, `.git`, wp-login/xmlrpc, phpMyAdmin, Spring Actuator, server-status, etc. |
| High-volume / enumeration / brute-force source-IP rollup | 1, 2 | in-model | Per-IP totals with `offender_threshold` and `error_threshold`. |
| Analyst report plus machine formats | 1, 3, 4 | in-model | `output` = report/table/json/csv/blocklist. |
| Live GeoIP/ASN reputation enrichment | 1, commercial tools | out-of-model | Requires external databases/network lookups; not part of pure offline gizza blocks. |
| Stateful banning / firewall changes | 2 | out-of-model | This repo produces local analysis text; it does not mutate system firewalls. |
| Full WAF rule engine parity | 3 | out-of-model | A complete CRS-compatible parser/scorer is much larger; this tool is deterministic triage. |

## Defaults chosen

- `category=all`, `min_severity=all`, `output=report` so a pasted sample gives an analyst summary immediately.
- `decode=true` because encoded payloads are table-stakes in web logs.
- `offender_threshold=20` and `error_threshold=5` catch noisy scans without flagging one-off normal users.
- `limit=500` keeps table/JSON/CSV output bounded while the report counts the full log.

## Worked example carried to surfaces

An Apache combined sample with SQLi, traversal, and encoded XSS produces a report headed like:

`Weblog attack analysis · combined · 3 requests · 3 flagged · 2 source IPs`

It then lists categories by severity, top offenders, and per-finding line/source/method/target/status/matched-signature details.

## Honest limits

- Heuristic signatures can false-positive and false-negative; the output is a triage queue, not a legal/security verdict.
- Logs behind reverse proxies need real client IP normalization before per-IP behaviour is meaningful.
- No GeoIP, ASN, reputation, or firewall mutation is attempted in this pure local block.

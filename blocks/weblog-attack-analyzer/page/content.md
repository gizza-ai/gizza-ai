## What this tool does

Paste Apache, Nginx, or IIS access logs and this tool highlights requests that look like web-application attack traffic. It parses each request, percent-decodes the target once and twice, matches a curated set of signatures, and then rolls findings up by source IP so you can see both individual payloads and noisy offenders.

It flags these classes:

- **SQL injection** — `UNION SELECT`, tautologies, time-delay probes, `information_schema`, `xp_cmdshell`, and similar payloads.
- **XSS** — encoded or raw `<script>`, event handlers, `javascript:` URLs, SVG/iframe/img payloads, and cookie-stealing probes.
- **Traversal and file inclusion** — `../`, `/etc/passwd`, Windows paths, PHP stream wrappers, and remote include attempts.
- **RCE and scanner traffic** — Log4Shell/JNDI payloads, shell command markers, and user agents such as sqlmap, nikto, nuclei, wpscan, and masscan.
- **Sensitive-path probes** — `.env`, `.git`, `wp-login.php`, phpMyAdmin, Spring Actuator, server-status, and other common reconnaissance paths.

The report also calls out source IPs with high request volume, many 404s (enumeration), or repeated 401/403s (brute-force style behaviour). Use the **Blocklist** output when you just want one suspicious IP per line.

## Worked example

Paste this sample with **Output as: Report**:

```text
203.0.113.5 - - [11/Mar/2024:09:14:02 +0000] "GET /products.php?id=1%27+UNION+SELECT+null,version()--+- HTTP/1.1" 200 512 "-" "sqlmap/1.7"
203.0.113.5 - - [11/Mar/2024:09:14:05 +0000] "GET /admin/../../etc/passwd HTTP/1.1" 404 153 "-" "sqlmap/1.7"
198.51.100.22 - - [11/Mar/2024:09:14:20 +0000] "GET /search?q=%3Cscript%3Ealert(1)%3C/script%3E HTTP/1.1" 200 980 "-" "Mozilla/5.0"
```

The output starts with a compact caption such as:

```text
Weblog attack analysis · combined · 3 requests · 3 flagged · 2 source IPs
```

Then it lists categories by severity, ranks the top source IPs, and shows each finding with the line number, source IP, request target, status code, and matched signature names.

## Limits and edge cases

- This is a **heuristic triage tool**, not a WAF, IDS, or legal/security determination. It points you to suspicious requests to investigate.
- It parses Apache/Nginx common and combined logs plus IIS W3C logs with a `#Fields:` header. Other shapes should be normalized first with a log parser.
- Encoded payload detection is on by default. Turn **Decode percent-encoded payloads** off only when you need to compare raw matching behaviour.
- Source-IP aggregation uses the IP printed in the log. If your logs contain a reverse proxy address instead of the real client IP, normalize `X-Forwarded-For` upstream first.

## FAQ

<details>
<summary>How is this different from the Log Analyzer tool?</summary>

Log Analyzer summarizes general log health: severity counts, top errors, time span, and volume. Weblog Attack Analyzer is security-focused: it looks specifically at access-log request targets, status codes, user agents, and source IP behaviour to flag attack attempts and scanning.

</details>

<details>
<summary>Does it upload my logs?</summary>

No. The standalone page runs the WebAssembly analyzer in your browser. Chat and CLI runs are local to the gizza runtime as well; there is no registry lookup, enrichment service, or remote scoring API.

</details>

<details>
<summary>Will this catch every attack?</summary>

No. It uses deterministic signatures and simple per-IP thresholds. It catches common probes and noisy scans well, but a targeted attacker can evade signatures or blend into normal volume. Treat the output as a prioritized review queue, then confirm with application logs, WAF events, and server context.

</details>

<details>
<summary>Why are percent-decoded matches important?</summary>

Attack payloads in URLs are often encoded, sometimes twice. For example `%3Cscript%3E` is `<script>` and `%252e%252e%252f` becomes `../` after two decode passes. The tool checks raw, once-decoded, and twice-decoded targets by default so those probes are not missed.

</details>

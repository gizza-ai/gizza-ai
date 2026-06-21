## About this tool

**Extract IP Addresses** scans pasted text or a log file and pulls out every
**IPv4** and **IPv6** address it contains — validated, deduplicated, and grouped
by version.

- **Validated**, not just pattern-matched: each candidate is parsed by a real IP
  parser, so `999.1.1.1` or a time like `12:34:56` is rejected.
- **Deduplicated**, in first-seen order. IPv6 addresses are normalized to their
  canonical compressed form (e.g. `2001:0db8:0000:…:0001` → `2001:db8::1`), so
  the same address written two ways counts once.
- **Robust to noise**: handles ports (`203.0.113.7:8080`), bracketed IPv6 in URLs
  (`http://[2001:db8::a]:443/`), and surrounding punctuation.

Everything runs **locally in your browser** via WebAssembly — your logs are never
uploaded.

### Handy for

- Pulling client/server IPs out of access or firewall logs.
- Building a unique IP list from a paste of mixed traffic.
- Quickly seeing whether a log contains IPv6 traffic at all.

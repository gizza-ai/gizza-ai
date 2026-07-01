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

## FAQ

<details>
<summary>Why doesn't 999.1.1.1 or a timestamp like 12:34:56 show up?</summary>

Because every candidate is validated by a real IP parser, not just a regex.
`999.1.1.1` fails because an IPv4 octet can't exceed 255, and `12:34:56` is
rejected because an IPv6 candidate must contain at least two colons and parse
as a genuine address.

</details>

<details>
<summary>The same IPv6 address appears twice in my log but only once here — why?</summary>

IPv6 addresses are normalized to their canonical compressed form before
deduplication, so `2001:0db8:0000:0000:0000:0000:0000:0001` and `2001:db8::1`
are recognized as the same address and counted once.

</details>

<details>
<summary>Does it handle addresses with ports or inside URLs?</summary>

Yes. `203.0.113.7:8080` yields `203.0.113.7`, and a bracketed IPv6 in a URL
like `http://[2001:db8::a]:443/` yields `2001:db8::a` — the port and
surrounding punctuation are stripped.

</details>

<details>
<summary>What order are the results in?</summary>

First-seen order within each group: all unique IPv4 addresses first, then all
unique IPv6 addresses, plus a total count. There's no sorting — the order
mirrors your log.

</details>

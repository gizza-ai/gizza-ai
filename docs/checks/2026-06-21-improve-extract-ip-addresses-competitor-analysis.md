# extract-ip-addresses — competitor analysis & differentiation

**Tool:** `gizza-ai/extract-ip-addresses` — find, validate and deduplicate all
IPv4 and IPv6 addresses in text or a log.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `grep -Eo '<ipv4-regex>'` | CLI | Standard move, but the regex matches `999.1.1.1`; no IPv6 (the IPv6 regex is famously horrible); no dedup/canonicalization without extra `sort -u`. |
| Online "extract IP from text" tools | Web | Often upload your log to a server, frequently IPv4-only, and rarely validate octets or canonicalize IPv6. |
| Log analyzers (GoAccess, etc.) | App | Heavyweight, aimed at full log analytics, not a quick "pull the IPs out of this paste". |
| Manual regex in code | DIY | IPv6 validation by regex is error-prone; most people get `::` compression wrong. |

## How gizza's tool is better / different

1. **Real validation, not regex guessing.** Candidates are parsed by the standard
   library's `IpAddr` parser, so invalid octets (`999.1.1.1`) and look-alikes
   (a time like `12:34:56`) are rejected — something a plain regex can't do.
2. **IPv6 done right.** Finds compressed (`::`), bracketed-in-URL
   (`[2001:db8::a]:443`) forms, and **canonicalizes** to the compressed
   representation so `2001:0db8:0000:…:0001` and `2001:db8::1` dedupe to one.
3. **Handles ports and punctuation.** `203.0.113.7:8080` yields `203.0.113.7`.
4. **Deduplicated, grouped, ordered.** Separate IPv4/IPv6 lists, first-seen
   order, unique.
5. **Local + three surfaces.** Chat ("pull the IPs from this log"), CLI, and a
   zero-upload page — one Rust core. Logs never leave the device.

## Verification

CLI run on *"client 192.168.0.1:443 -> 10.0.0.5, dup 192.168.0.1, v6
2001:0db8::1 and [fe80::a]:22, time 12:34:56"* returned exactly
`ipv4: [192.168.0.1, 10.0.0.5]`, `ipv6: [2001:db8::1, fe80::a]` — port stripped,
IPv6 canonicalized, bracket form handled, the time rejected, and the duplicate
IPv4 collapsed.

## Scope / honest limitations

- Reports addresses, not CIDR ranges or netmasks (those could be a future add).
- Doesn't classify private vs public / reserved ranges (possible enhancement).

## Possible future enhancements

- Tag each address as private/public/reserved/loopback.
- Optional CIDR-range extraction.
- Counts-per-address (frequency) for log triage.

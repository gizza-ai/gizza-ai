# Competitor analysis: ip-address-validate

Date: 2026-08-23

## Sources reviewed

- IPVoid / online IP validators: single-address validation plus category/security-oriented flags.
- MXToolbox / DNS-style IP tools: canonical parsing, reverse-DNS-oriented output, and clear invalid reasons.
- Browser/utility IPv4/IPv6 validators and CIDR calculators: bulk text input, IPv6 compression/expansion, private/loopback/multicast labels.

## Table-stakes capabilities

| Capability | In model? | Decision |
| --- | --- | --- |
| Validate IPv4 and IPv6 syntax | Yes | Core parser handles both families and returns line-specific errors. |
| Canonical IPv6 compression/expansion | Yes | `ipv6_form` enum selects RFC 5952 compressed or fully expanded. |
| Category labels (private, loopback, link-local, multicast, documentation, etc.) | Yes | Implemented locally from address ranges; no network calls. |
| Bulk paste | Yes | One address per line, blank lines ignored, capped at 5,000 lines / 500 KB. |
| CSV/JSON/export-friendly output | Yes | `output` enum includes report/table/json/valid/invalid/summary. |
| CIDR prefix and port suffix handling | Yes | Optional `allow_prefix` / `allow_port` checks with family-specific bounds. |
| Reverse DNS pointer | Yes | Included in table/json/report output. |
| Reachability, WHOIS, reputation, geolocation | Out of model | Requires network data; explicitly not built. |
| DNS resolution of hostnames | Out of model | This is an IP syntax tool only; hostnames are rejected. |

## UX decisions

- Textarea input with worked examples for mixed lists, IPv6 canonicalization, invalid-line cleanup, CSV audits and summaries.
- Select controls for accepted family, output shape and IPv6 spelling.
- Checkboxes for prefix, port, leading-zero and dedupe behavior.
- Page and CLI examples avoid any branded copy and emphasize local, no-network validation.

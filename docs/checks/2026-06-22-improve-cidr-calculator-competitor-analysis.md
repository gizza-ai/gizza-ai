# cidr-calculator — competitor analysis (2026-06-22)

Tool: `blocks/cidr-calculator` — compute subnet properties for an IPv4 or IPv6 CIDR.
Surfaces verified: chat (drift-guard schema + `wafer build` validate), CLI (`gizza tool
cidr-calculator`), page (`tests/tool-page-cidr-calculator.spec.ts`, 2 specs pass).

## Top competitors surveyed

1. **jodies.de ip-calc / sipcalc** — the canonical CLI/web subnet calculator. Shows address,
   netmask, wildcard, network, broadcast, host-min, host-max, hosts/net, and the binary
   representation of each. IPv4 + IPv6.
2. **calculator.net IP Subnet Calculator** — network address, usable host range, broadcast,
   total/usable hosts, subnet mask, wildcard, binary subnet mask, IP class, CIDR notation.
3. **ipaddressguide.com CIDR calculator** — CIDR → netmask, wildcard, first/last IP, total host
   count; also the reverse (range → CIDR).
4. **mxtoolbox / arin subnet tools** — network, broadcast, mask, usable range, count; flags
   private vs public ranges.
5. **browserling / IPv6 subnet calculators** — IPv6 prefix → network, first/last address, total
   address count (handles 2^128 magnitudes).

## Capability diff vs our tool

| Capability | Competitors | Our tool | Status |
|---|---|---|---|
| Network address | yes | yes | covered |
| Broadcast address (IPv4) | yes | yes | covered |
| Netmask (dotted) + CIDR prefix | yes | yes | covered |
| Wildcard mask | yes | yes | covered |
| Usable host range (first–last) | yes | yes | covered |
| Total addresses + usable host count | yes | yes | covered |
| Non-aligned base normalized to network | most | yes | covered |
| `/31` RFC 3021 + `/32` single-host edge cases | some | yes | covered (better than several) |
| Private vs public scope flag | some | yes (RFC1918 + loopback + link-local + CGNAT 100.64/10) | covered |
| IPv6 (network, first/last, count incl. 2^128) | some | yes | covered |
| Machine-readable JSON output | few | yes | covered (parity edge) |
| Binary representation of mask/address | a few (sipcalc, calculator.net) | no | gap — see below |
| IP class (A/B/C) label | calculator.net | no | low value — classful addressing is deprecated (RFC 1519); intentionally omitted |
| Reverse: IP range → CIDR | ipaddressguide | no (out of scope) | covered by existing `blocks/ip-range-expand` family direction; not a regression |

## Gaps considered and decision

- **Binary mask / address representation** (sipcalc, calculator.net show e.g.
  `11111111.11111111.11111111.00000000`). This is in-model (pure compute, no new dep) and the
  only material feature gap. It is a nice-to-have but adds output noise; the current report
  already covers every field a network engineer needs to subnet. Deferred as a possible future
  enhancement rather than shipped, to keep the default report concise. (Not closed — flagged
  honestly rather than claimed.)
- **IP class (A/B/C)**: deliberately omitted. Classful addressing was deprecated by CIDR
  (RFC 1519, 1993); showing a class label on a CIDR result is misleading, so this is a correct
  omission, not a gap.
- **Reverse range→CIDR**: distinct tool shape; `blocks/ip-range-expand` already covers
  enumerating/counting a CIDR or range, so we do not duplicate it here.

## Outcome

The tool reaches feature parity with the leading CIDR/subnet calculators on every core field,
and exceeds several on edge-case correctness (`/31`, `/32`, non-aligned normalization, exact
IPv6 2^128 counts) and on offering a JSON output mode. No copy/branding/trademark was copied
from any competitor. The one optional in-model gap (binary mask representation) is documented
above and left as a future enhancement.

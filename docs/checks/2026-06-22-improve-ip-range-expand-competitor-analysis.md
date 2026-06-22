# ip-range-expand — competitor analysis (2026-06-22)

Tool: expand a CIDR block or a `start-end` IP range into the full address list
(or a count), for IPv4 and IPv6. Surfaces: chat skill (`gizza-ai/ip-range-expand`),
CLI (`gizza tool ip-range-expand`), and the `/tools/ip-range-expand/` page.

## Top competitors surveyed

1. **IPLocate.io — CIDR to IP Range Converter** — CIDR ↔ range; IPv4 + IPv6.
2. **VariedTools — CIDR Expander** — IPv4/IPv6 CIDR → normalized CIDR, first/last
   address, network type.
3. **Orbit2x — IP Range Expander** — IPv4 + IPv6, paginates large ranges.
4. **Zecurit — IPv4 Range Expander** — CIDR, dash range, AND wildcard notation
   (IPv4 only; IPv6 is a separate tool).
5. **FreeWWW — CIDR Range Expander & Subnet Calculator** — expand, subnet split,
   membership check, network compare, binary view, export. IPv4 + IPv6.
6. **IPAddressGuide / cidr.xyz / EaseCloud / AnyTools** — CIDR→range, subnet mask
   input, first/last/usable-host counts, visual address-space view.

Sources:
- https://www.iplocate.io/tools/cidr-to-ip-range-converter
- https://www.variedtools.com/expand-cidr-to-range
- https://orbit2x.com/ip-range-expander
- https://zecurit.com/tools/ipv4-range-expander/
- https://www.freewww.com/apps/cidr/
- https://www.ipaddressguide.com/cidr
- https://cidr.xyz/
- https://www.easecloud.io/tools/network/ipv4-range-expander/
- https://www.anytools.work/en/network/ipv4-range-expander

## Capability diff (ranked, fit-to-model)

| Capability | Competitors | gizza ip-range-expand | Verdict |
| --- | --- | --- | --- |
| Expand IPv4 CIDR → full list | all | yes | parity |
| Expand IPv6 CIDR → full list | most | yes | parity |
| `start-end` range (dash) | Zecurit, Orbit2x, IPLocate | yes (IPv4 + IPv6) | parity |
| Bare-suffix range end (`.10-20`) | some | yes (IPv4) | parity+ |
| Count addresses (incl. huge IPv6 /64, /0) | few do this exactly | yes, exact unbounded (string math) | **parity+** |
| Non-aligned base normalized to network | most | yes (`192.168.1.130/29`→`.128`) | parity |
| Includes network + broadcast addresses | most | yes (whole block) | parity |
| Output size guard for huge ranges | Orbit2x paginates; FreeWWW counts | `limit` param + clear over-limit error | parity (model-fit) |
| Wildcard notation (`192.168.1.*`) | Zecurit only | no | gap — IN model, low value (see below) |
| Subnet-mask input (`/255.255.255.0`) | EaseCloud, IPAddressGuide | no | gap — IN model, low value |
| First / last / usable-host summary | IPAddressGuide, cidr.xyz | no (list+count cover it) | minor, partial overlap |
| Membership check / network compare | FreeWWW | no | OUT of scope (different tool) |
| Binary / visual address-space view | FreeWWW, cidr.xyz | no | OUT of model (no canvas/UI surface) |
| Subnet split into smaller blocks | FreeWWW | no | OUT of scope (different tool) |

## Gaps closed this build

- **Exact count for arbitrarily large ranges**, including IPv6 `/64`
  (18 446 744 073 709 551 616) and `/0` (2^128) — handled with string-based
  count so the value never overflows `u128`. Several competitors cannot show an
  exact count for a `/0`. (Verified by unit tests `count_v6_huge`.)
- **Output guard**: `list` refuses to enumerate beyond `limit` (default 65536)
  and reports the true size, steering the user to `count` — a cleaner answer than
  a frozen browser or silent truncation. Counting always ignores the limit.
- **Three honest input forms** documented and tested: CIDR, full dash range, and
  IPv4 bare-suffix range. IPv4 + IPv6 throughout.
- Page renders `output` as a `<select>` and `limit` as a numeric field; CLI and
  chat share the same descriptor schema (drift-guarded).

## Deliberately NOT built (out of model / scope, per skill rules)

- **Visual / binary address-space view** and **interactive CIDR slider**
  (cidr.xyz, FreeWWW): need a custom canvas/interactive UI the page driver
  (single text output) does not provide.
- **Membership check, network compare, subnet split**: these are distinct tools,
  not the "expand a range" job; bundling them would muddy the single-purpose
  descriptor. Candidates for separate backlog tools.

## Low-value in-model gaps (left for a future pass)

- **Wildcard notation** (`192.168.1.*`) and **subnet-mask input**
  (`192.168.1.0/255.255.255.0`): both are buildable in the pure-Rust core (just
  extra parse branches), but they are minor conveniences over the two forms
  already supported and only one surveyed competitor offers each. Noted for a
  follow-up; not blocking parity.

## Verification (this build)

- `cargo test --workspace` in `blocks/ip-range-expand/`: 17 core + 1 drift-guard
  schema test pass.
- CLI: `gizza tool ip-range-expand` verified for IPv4/IPv6 CIDR list, count,
  dash range, IPv6 `/64` count, over-limit error, and invalid input.
- Page: Playwright `tool-page-ip-range-expand.spec.ts` (2 specs) pass headless —
  CIDR list, dash range, count select, IPv6 `/64` count, IPv6 CIDR list.

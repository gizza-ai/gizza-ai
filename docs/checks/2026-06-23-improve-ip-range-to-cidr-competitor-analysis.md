# ip-range-to-cidr — competitor analysis & improvement snapshot (2026-06-23)

Tool: convert an arbitrary inclusive `start-end` IP range into the **minimal**
set of CIDR blocks that exactly covers it (no extra or missing addresses), for
both IPv4 and IPv6. Inverse of `ip-range-expand`.

## Surfaces verified

- **chat block** — `wafer build` OK, instantiates clean (306.8 KiB), drift-guard
  schema test passes.
- **CLI** — `gizza tool ip-range-to-cidr input=… output=…` verified for IPv4
  list, IPv4 count, IPv6 list, and bare-octet shorthand.
- **page** — `/tools/ip-range-to-cidr/`, Playwright `tool-page-ip-range-to-cidr.spec.ts`
  (2 tests) passes: unaligned→minimal list, aligned→single CIDR, count mode, IPv6.
- **unit** — 17 core tests (alignment, unaligned split exactness, single-host,
  shorthand, whole-space /0, top-of-space no-overflow, IPv6, errors) + 1 drift test.

## Competitors surveyed (top 5)

| Tool | Core (minimal CIDR cover) | IPv4 | IPv6 | Per-CIDR count | Total count | Visual | Notes |
|------|---------------------------|------|------|----------------|-------------|--------|-------|
| ip2cidr.com | yes | yes | yes? | – | – | – | minimal block list; sparse UI |
| ipgeolocation.io (range-to-cidr) | yes ("fewest possible CIDR blocks that exactly cover the range") | yes | yes | yes (per-block address counts) | – | – | heavy educational/FAQ copy |
| networkingtoolbox.net | yes ("smallest number of CIDR blocks", "all addresses in range are included") | yes | yes | – | – | – | aligned blocks; firewall/ACL framing |
| ipaddressguide.com/cidr | range↔CIDR both ways | yes | – | – | yes | – | IPv4 only; bidirectional |
| cidr.xyz | CIDR/range/mask compute | yes | yes | – | yes | yes (interactive subnet viz) | a CIDR explorer, not range→CIDR-first |

## Gap analysis (fit-to-model)

In-model gaps — **all closed**:
- **Minimal/exact cover** (the table-stakes capability) — implemented via the
  standard greedy aligned-block algorithm; `cover_is_exact_for_random_ish_ranges`
  test proves the cover is contiguous and ends exactly at `end+1` (no over/under).
- **IPv4 + IPv6** — both families supported; whole-space `/0` and top-of-space
  edge cases covered without overflow.
- **Count output** — `output=count` returns the number of CIDR blocks, matching
  the "how many prefixes" question competitors surface.
- **Single-address / shorthand convenience** — a bare address → `/32`/`/128`
  host route; IPv4 bare-final-octet shorthand (`192.168.1.10-20`), which most
  competitors do not accept, is a small UX edge in our favour.
- **Clear errors** — reversed range, mixed-family endpoints, and garbage all
  produce actionable messages rather than silent wrong output.

Out-of-model features (NOT built — listed for honesty, not copied):
- **Per-CIDR address counts** (ipgeolocation) — the page/CLI return a plain text
  block list; annotating each line with its host count would change the output
  contract and the inverse-symmetry with `ip-range-expand`. The existing
  `cidr-calculator` tool already gives per-block address counts, and `count` mode
  here gives the block count, so this is covered across the toolset rather than
  inline. Deferred to avoid output drift.
- **Interactive subnet visualisation** (cidr.xyz) — out of scope for the
  text-output page driver (no live canvas widget in the page model).
- **Bidirectional CIDR→range** (ipaddressguide) — that is the inverse direction
  and is already the existing `ip-range-expand` + `cidr-calculator` tools; building
  it here would duplicate them.

## Copy / SEO

- Title/description/H1/tags written for the "IP range to CIDR" / "CIDR
  aggregator" / "range to CIDR" intent; no competitor branding, copy, or
  trademarks reused.
- `page/content.md` explains the minimal-cover concept, input formats, output
  modes, and the firewall/ACL/route-table use case in original wording.

## Conclusion

The tool matches every in-model capability of the surveyed competitors (minimal
exact IPv4+IPv6 cover, block count) and adds shorthand input and strict error
handling. Remaining competitor extras (per-line counts, visualisation,
CIDR→range) are either out of the page model or already covered by sibling tools
(`ip-range-expand`, `cidr-calculator`), so no in-model gap remains.

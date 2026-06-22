# mac-vendor-lookup — competitor analysis (2026-06-22)

`/create-next-tool` backlog pick. Pure-Rust tool: the full IEEE MA-L OUI registry
(39,572 assignments) is bundled into the wasm via `include_str!` of a pre-sorted
`prefix\torg` TSV and binary-searched at runtime. No I/O, so it runs on **all**
surfaces: chat (block.wasm 1.47 MiB instantiates), CLI, and the standalone page.

## Competitors surveyed (paraphrased)
| tool | does well | dimension |
| ---- | --------- | --------- |
| Wireshark OUI lookup | authoritative IEEE data, simple prefix search | capabilities |
| macvendors.com | IEEE-sourced, updated daily, free REST API, 16.5k+ vendors | capabilities / freshness |
| macaddress.io | OUI + IAB lookup, VM detection, decodes the U/L & I/G bits, location | capabilities |
| maclookup.app / oui.is | MA-L/MA-M/MA-S/IAB + CID registries, format parsing | capabilities |
| MockAddress | format parsing/conversion, validation, vendor details | UX / capabilities |
| iotools / whatsmydns | bulk (multiple addresses at once), reverse vendor→OUI | UX / capabilities |

## Gap diff vs our tool
Our tool: parse any common MAC/EUI form (colon, hyphen, Cisco dot, bare hex,
upper/lower, full address or OUI-only, up to EUI-64), look the OUI up in the
bundled IEEE registry, and return the normalized MAC, the OUI, the vendor (or an
"unassigned" note), and the decoded U/L (global vs locally administered) and I/G
(unicast vs multicast) bits.

**Gaps closed this iteration:**
- **Batch / bulk lookup** (matches iotools, whatsmydns): paste several addresses
  one per line → one compact `MAC — Vendor` line each; single address keeps the
  detailed multi-line block. Page field is now `multiline` (textarea) so pasted
  newlines survive.
- **U/L & I/G bit decoding** (matches macaddress.io): the "Type:" line reports
  globally-unique vs locally-administered and unicast vs multicast — and explains
  why randomized phone MACs aren't found.

**In-model gaps considered, deferred (good follow-ups):**
- **MA-M / MA-S / IAB sub-registries** — these are 28-/36-bit assignments inside a
  shared MA-L OUI; resolving them needs the extra IEEE files and a longest-prefix
  match. The bundled MA-L (the 24-bit registry) covers the overwhelming majority
  of consumer/network gear; adding MA-M/S would grow the wasm and the parser.
- **Reverse lookup (vendor name → OUIs)** — a substring scan of the registry;
  a clean separate mode, deferred to keep one focused input.
- **Registry freshness** — competitors fetch daily; ours is a point-in-time
  snapshot of the IEEE MA-L CSV bundled at build time (the offline trade-off). A
  rebuild refreshes it. This is inherent to the "fully offline, no network" model.

**Out-of-model:** live REST API (we're a chat/CLI/page tool, not a hosted API);
geolocation of a vendor (not in the IEEE data); VM-vendor heuristics beyond the
registry name.

## Tested
unit (13: vendor resolve in colon/hyphen/dot/bare forms, OUI-only, U/L & I/G bit
detection, unknown-OUI → no vendor, batch one-line-per-address + blank-line skip,
report single vs error, and the 4 input-validation error paths) + drift-guard
schema test · `cargo test --workspace` green · `wafer build` validates the chat
block.wasm (pure-Rust → also runs in the chat SW) · CLI on single + Cisco-dot +
bad input + a 3-address batch · Playwright (3 specs: known vendor, Cisco dotted
form, unassigned OUI + bad input) green.

> Original work only — no competitor copy, branding, or trademarks copied. The
> OUI data is the public IEEE MA-L registry.

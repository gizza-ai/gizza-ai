# parse-tcp-header — competitor analysis (2026-06-21)

## Tool

`parse-tcp-header` decodes a raw TCP segment header given as a hex string into
every header field. Three surfaces verified: chat skill block (`wafer build`
validated, JSON out), CLI (`gizza tool parse-tcp-header header=…`, JSON out),
and the standalone page (`/tools/parse-tcp-header/`, human-readable text out,
Playwright-verified). Pure-Rust → runs on all backends, nothing uploaded.

## Competitors surveyed

1. **Teleport "Decode TCP Header"** (goteleport.com) — single-layer decoder
   focused on the TCP header; pastes hex and shows the standard fields.
2. **Packetor** (packetor.com) — full multi-layer packet decoder backed by the
   tshark engine; decodes Ethernet/IP/TCP/UDP and renders a Wireshark-style
   field tree (TCP is one layer among many).
3. **Hex Packet Decoder** (hpd.gasmi.net) — paste a hex dump, force the base
   layer, renders a layered field breakdown including the TCP layer.
4. **CalculateYogi Hex Packet Decoder** — decodes Ethernet, IPv4, TCP, UDP,
   ICMP, ARP from a hex dump.
5. Various reference walkthroughs (RFC 9293 / RFC 793, blog explainers) of the
   manual TCP-header decode — reference material, not interactive tools.

Sources:
- [Teleport — Decode TCP Header](https://goteleport.com/resources/tools/decode-tcp-header/)
- [Packetor](https://packetor.com/)
- [Hex Packet Decoder (gasmi.net)](https://hpd.gasmi.net/)
- [CalculateYogi Hex Packet Decoder](https://calculateyogi.com/technology/hex-packet-decoder)
- [RFC 9293 — Transmission Control Protocol](https://www.rfc-editor.org/rfc/rfc9293)

## Capability diff (in-model gaps closed)

| Field / feature | Competitors | parse-tcp-header | Status |
|---|---|---|---|
| Source / destination ports | yes | yes | covered |
| Sequence number (dec + hex) | dec | yes (both) | ahead |
| Acknowledgement number (dec + hex) | dec | yes (both) | ahead |
| Data offset (words + bytes) | words | yes (both) | ahead |
| Reserved bits | partial | yes | covered |
| Flags NS/CWR/ECE/URG/ACK/PSH/RST/SYN/FIN | yes | yes (all 9) | covered |
| Compact set-flags list (e.g. "SYN ACK") | varies | yes | covered |
| Window size | yes | yes | covered |
| Checksum (stored value) | yes | yes | covered |
| Urgent pointer | yes | yes | covered |
| TCP options parsed + named | partial (raw) | yes (MSS/WScale/SACK/SACK-Perm/TS/NOP/EOL) | ahead |
| Option decoded value (MSS, Window Scale) | rarely | yes | ahead |
| Lenient input (spaces/colons/dashes/dots/0x) | varies | yes | covered |
| Privacy — runs locally, no upload | server-side | yes (in-browser/local) | ahead |

All competitor TCP-header capabilities are matched, plus several are ahead:
named + length-aware TCP option parsing with decoded MSS/Window-Scale values,
both decimal and hex for the 32-bit seq/ack numbers, data offset in both words
and bytes, a compact set-flags summary, and fully local execution.

## Out-of-model features (intentionally not built)

- **Checksum validity recompute.** The TCP checksum covers the IPv4/IPv6 pseudo
  header (source/dest address, protocol, TCP length) plus the TCP segment and
  payload — none of which are present in a TCP-header-only hex input. Validating
  it would require the IP addresses and the full payload, which is a different
  input contract. The stored checksum value is shown; validity is correctly left
  out rather than computed from incomplete data. (The sibling
  `parse-ipv4-header` recomputes the IPv4 checksum, which *is* self-contained.)
- **Multi-layer decode** (Ethernet → IP → TCP payload tree, à la Packetor /
  tshark). This tool is scoped to the TCP header only by design; the siblings
  `parse-ethernet-frame` and `parse-ipv4-header` cover the lower layers.
- **Live capture / pcap upload.** Input is a hex string, matching the chat/CLI
  text contract; the `parse-pcap` sibling handles capture files.

## Copy / UX / visual

- Title, hero, SEO tags, and `content.md` written fresh — no competitor copy,
  branding, or trademarks reused.
- Page renders an aligned, labelled field list with an options breakdown;
  placeholder shows a real example SYN header. Input is `multiline = true` so
  pasted multi-line hex dumps are preserved.

## Test matrix (all green)

- `cargo test --workspace` — 15 tests (core happy/error paths: SYN, SYN-ACK,
  FIN/PSH/ACK, options MSS+WScale, SACK+Timestamps, NS flag, render + JSON, and
  the drift-guard schema test). Pass.
- `wafer build` — chat block compiles + validates (307.9 KiB). Pass.
- `wasm-pack build …/web` — page wasm built. Pass.
- `gizza tool parse-tcp-header header=…` — JSON out correct (ports, SYN, MSS=1460,
  Window Scale=7). Pass.
- Playwright `tool-page-parse-tcp-header.spec.ts` — 2 specs (ports/flags/window,
  and options MSS + Window Scale). Pass.

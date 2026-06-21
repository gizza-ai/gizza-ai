# parse-ipv4-header — competitor analysis (2026-06-21)

## Tool

`parse-ipv4-header` decodes a raw IPv4 packet header given as a hex string into
every header field. Three surfaces verified: chat skill block (`wafer build`
validated, JSON out), CLI (`gizza tool parse-ipv4-header header=…`, JSON out),
and the standalone page (`/tools/parse-ipv4-header/`, human-readable text out,
Playwright-verified). Pure-Rust → runs on all backends, nothing uploaded.

## Competitors surveyed

1. **Packetor** (packetor.com) — full multi-layer packet decoder backed by the
   tshark engine; decodes Ethernet/IP/TCP/UDP/etc. and renders a Wireshark-style
   field tree.
2. **Hex Packet Decoder** (hpd.gasmi.net) — pastes a hex dump, force-selects the
   base layer (e.g. `force=ipv4`), renders a layered field breakdown.
3. **CalculateYogi Hex Packet Decoder** — decodes Ethernet, IPv4, TCP, UDP,
   ICMP, ARP from a hex dump.
4. **Teleport "Decode TCP Header"** — single-layer decoder focused on the TCP
   header (sibling problem; TCP not IPv4).
5. Various blog walkthroughs (xerocrypt, DaniWeb) explaining the manual
   nibble-by-nibble IPv4 decode — reference material, not tools.

Sources:
- [Packetor](https://packetor.com/)
- [Hex Packet Decoder (gasmi.net)](https://hpd.gasmi.net/)
- [CalculateYogi Hex Packet Decoder](https://calculateyogi.com/technology/hex-packet-decoder)
- [Teleport — Decode TCP Header](https://goteleport.com/resources/tools/decode-tcp-header/)
- [How to read IP packets (hex) manually — DaniWeb](https://www.daniweb.com/hardware-and-software/networking/threads/510419/how-to-read-ip-packets-hex-manually)

## Capability diff (in-model gaps closed)

| Field / feature | Competitors | parse-ipv4-header | Status |
|---|---|---|---|
| Version / IHL (words + bytes) | yes | yes | covered |
| DSCP + ECN, named classes (CS/EF/AF) | partial (raw ToS) | yes, named | covered, ahead |
| Total length + implied payload length | total only | yes (both) | ahead |
| Identification (dec + hex) | yes | yes | covered |
| Flags DF/MF + reserved | yes | yes | covered |
| Fragment offset (units + bytes) | offset only | yes (both) | ahead |
| TTL | yes | yes | covered |
| Protocol number, named (TCP/UDP/ICMP/GRE/ESP/…) | yes | yes (~35 names) | covered |
| Header checksum **validity recompute** (RFC 1071) | varies | yes | covered |
| Source / destination dotted-decimal | yes | yes | covered |
| IP options bytes (IHL > 5) | yes | yes (hex) | covered |
| Lenient input (spaces/colons/dashes/dots/0x) | yes | yes | covered |
| Privacy — runs locally, no upload | server-side | yes (in-browser/local) | ahead |

All competitor IPv4-header capabilities are matched, plus several are ahead:
named DSCP/ECN classes, both unit + byte fragment offset, implied payload
length, a recomputed checksum-validity flag, and fully local execution.

## Out-of-model features (intentionally not built)

- **Multi-layer decode** (Ethernet → IP → TCP/UDP payload tree, à la Packetor /
  tshark). This tool is scoped to the IPv4 header only by design. The sibling
  `parse-ethernet-frame` covers the link layer; a TCP/UDP header parser would be
  a separate tool. Building an embedded tshark/dissector is out of model (no
  external engine; pure-Rust block).
- **Live capture / pcap upload.** Input is a hex string, matching the chat/CLI
  text contract; binary pcap parsing is a different tool shape.

## Copy / UX / visual

- Title, hero, SEO tags, and `content.md` written fresh — no competitor copy,
  branding, or trademarks reused.
- Page renders an aligned, labelled field list; placeholder shows a real example
  header. Input is `multiline = true` so pasted multi-line hex dumps are
  preserved.

## Test matrix (all green)

- `cargo test --workspace` — 13 tests (core happy/error paths + render + JSON +
  the drift-guard schema test). Pass.
- `wafer build` — chat block compiles + validates (301.9 KiB). Pass.
- `wasm-pack build …/web` — page wasm built. Pass.
- `gizza tool parse-ipv4-header header=…` — JSON out correct (TCP, valid
  checksum, 192.168.0.104 → 192.168.0.1). Pass.
- Playwright `tool-page-parse-ipv4-header.spec.ts` — 2 specs (TCP+DF+checksum,
  and DSCP-EF + MF-fragment + UDP). Pass.

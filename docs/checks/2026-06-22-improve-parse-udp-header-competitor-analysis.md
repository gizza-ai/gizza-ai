# parse-udp-header — competitor analysis (2026-06-22)

## What we built

A UDP datagram header decoder (RFC 768). Input: a hex string (the 8-byte header;
trailing payload bytes ignored; spaces/colons/dashes/dots/`0x` tolerated).
Output:

- source / destination ports
- well-known service name per port (DNS, NTP, DHCP, TFTP, SNMP, QUIC/HTTP-3,
  IKE/IPsec, syslog, RIP, OpenVPN, L2TP, RADIUS, SSDP, mDNS, WireGuard, …)
- total datagram **length** (8-byte header + data)
- implied **payload length** (length − 8)
- **checksum** as `0xNNNN`
- **checksum_disabled** flag (0x0000, legal over IPv4)

Three surfaces: chat skill (JSON), CLI (`gizza tool parse-udp-header header=…`),
and the in-browser page (human-readable render). All run locally; nothing is
uploaded.

## Top competitors surveyed

UDP-specific "paste-hex header decoder" tools are rare — most network-header
decoders are general packet dissectors. Closest comparators:

1. **Wireshark / tshark** — the reference decoder. Dissects UDP from a real
   capture: src/dst port (with service resolution), length, checksum (with
   validity verification against the pseudo-header), and stream/conversation
   tracking. Desktop app / pcap-oriented, not a paste-a-hex-string web tool.
2. **scapy** (`UDP(hex_bytes).show()`) — Python library; same four fields plus
   programmable. Not a web tool; requires a Python install.
3. **HPD / "Hex Packet Decoder" (online)** — paste a full packet hex dump,
   decodes Ethernet/IP/UDP layers including the UDP header fields. Decodes the
   whole stack, not the UDP header in isolation; expects the full frame.
4. **online "TCP/UDP header" educational decoders / university tools** — show
   the four UDP fields from a hex header. Typically no service-name annotation,
   no payload-length derivation, no disabled-checksum note.
5. **CyberChef** — has "Parse IP packet" / generic parse recipes, but no
   dedicated standalone UDP-header field breakdown; you assemble it from
   "From Hex" + manual offsets.

## Gap diff and decisions

In-model gaps closed (already in the shipped build):

- **Service-name annotation per port** — most simple decoders omit this; we add
  it for both source and destination (matches Wireshark's port→service
  resolution, the most useful enrichment for reading a header).
- **Payload length derivation** (length − 8) — competitors usually print only
  the raw length field; deriving the data size is the common follow-up question.
- **Disabled-checksum flag** (0x0000) — a real UDP/IPv4 nuance that bare decoders
  miss; we surface it explicitly.
- **Lenient input** — spaces, colons, dashes, dots, `0x` prefix all tolerated,
  and trailing payload bytes are ignored (paste the whole datagram).
- **JSON + human-readable** dual output across chat/CLI/page.

Out-of-model / deliberately not built (would require more than a hex header or
state gizza's local model can't carry):

- **Checksum verification.** UDP's checksum covers a pseudo-header built from the
  source/destination **IP addresses** and protocol — those bytes are not in the
  UDP header, so verifying (vs. merely reporting) the checksum would require the
  IP addresses as additional input. Wireshark can because it has the IP layer
  from the capture. We report the stored value and the disabled flag; we do not
  claim validity. (A future enhancement could accept optional src/dst IP params
  to compute and verify the checksum — noted, not built, to keep one clean
  hex-in input.)
- **Full-stack dissection** (Ethernet/IP framing, stream reassembly) — that is a
  separate concern already covered by sibling tools (`parse-ethernet-frame`,
  `parse-ipv4-header`, `parse-tcp-header`, `parse-pcap`); this tool decodes the
  UDP header in isolation, consistent with that family.

## Verification (all green)

- `cargo test --workspace` — 14 core unit tests + chat-schema drift-guard pass.
- `wafer build` — block.wasm validates (`gizza-ai/parse-udp-header v0.1.0`).
- `wasm-pack build …/web` — page wasm built.
- CLI: `gizza tool parse-udp-header header='c3d2 0035 0028 1b6e'` → correct JSON
  (src 50130, dst 53/DNS, length 40, payload 32, checksum 0x1b6e).
- Playwright `tool-page-parse-udp-header.spec.ts` — 2 tests pass (field decode +
  disabled-checksum note).

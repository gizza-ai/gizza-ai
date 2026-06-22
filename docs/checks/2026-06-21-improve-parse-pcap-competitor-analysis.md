# parse-pcap — competitor analysis (2026-06-21)

## What this tool does
Parses an uploaded libpcap (`.pcap`) or pcapng (`.pcapng`) capture into a per-packet
summary. Pure-Rust, dependency-free binary parsing (runs on every backend incl. the chat
Service Worker). Decodes:

- **Containers:** classic pcap (big-/little-endian, microsecond + nanosecond magic) and
  pcapng (SHB/IDB + Enhanced, Simple, and legacy Packet Blocks, honouring per-interface
  `if_tsresol` timestamp resolution).
- **Link layer:** Ethernet II (incl. 802.1Q/802.1ad VLAN tags), raw-IP, Linux cooked
  (SLL), BSD null/loopback.
- **Network/transport:** IPv4, IPv6 (RFC-5952 `::` compression), TCP (named flags), UDP,
  ICMP/ICMPv6, ARP (request/reply text), plus protocol naming for GRE/ESP/AH/OSPF/SCTP/…

Output (flat JSON the LLM reads directly): `format`, `link_type`, `total_packets`,
`returned_packets`, `truncated`, and a `packets[]` list — each with `index`, `timestamp`
(epoch seconds), on-wire `length` + `captured` bytes, `source`/`destination`,
`source_port`/`destination_port`, `protocol`, and a one-line `info`.

## Surfaces
- **Chat / LLM API:** yes (wafer-validated `block.wasm`, 508 KiB).
- **CLI:** yes — verified live against
  `https://raw.githubusercontent.com/the-tcpdump-group/tcpdump/master/tests/dhcp-rfc3004.pcap`
  (correct DHCP UDP 67/68 decode, timestamps, truncation).
- **Page:** none — a file-input → JSON report fits neither the pure-text page nor the
  ffmpeg file→media page shape (the established "no-page file-input" pattern, like
  `detect-file-type` / `web-fetch` / `pdf-extract-text`).

## Competitors surveyed
| Tool | Model | Notable features |
| --- | --- | --- |
| [A-Packets](https://apackets.com/) | Web upload (free ≤25 MB, public result page) | HTTP/DNS/FTP/Telnet reconstruction, extracted files & credentials, host graph |
| [Red Hand Analyzer](https://redhand.io/analyzer) | Web upload | Interactive behaviour report, exposed services, threat-intel matches |
| [PacketTotal](https://www.packettotal.com/) | Web upload | Malware detection, traffic visualisation, signature analytics |
| [ChatTCP](https://chattcp.com/en/pcap-online-analysis) | Web upload (≤50 MB) | AI TCP-problem diagnosis, per-packet TCP/UDP view |
| [Gigasheet PCAP viewer](https://www.gigasheet.com/popular-tools/pcap-file-viewer) | Cloud (≤250 GB) | Spreadsheet-style filtering/pivoting over parsed packets |

## Gap analysis (fit-to-model)

**Closed / at parity (in-model):**
- Both classic-pcap and pcapng containers (most lightweight parsers do pcap only).
- Per-packet L2/L3/L4 decode with addresses, ports, protocol, timestamp, lengths — the
  core table every competitor shows.
- TCP flag names, ARP request/reply text, IPv6 canonical compression, VLAN unwrapping.
- A `limit` cap with an always-accurate `total_packets`, so an LLM gets a bounded sample
  of an arbitrarily large capture without OOM (the chat/LLM-native angle competitors lack —
  they are web dashboards, we are a callable tool + CLI).

**Out-of-model (deliberately not built — would need stateful flow reassembly, an ML/threat
feed, or a UI we don't have):**
- Application-layer reconstruction (HTTP/DNS/TLS object & file extraction) — needs TCP
  stream reassembly across packets; large surface, separate tool.
- Malware / threat-intelligence scoring (PacketTotal, Red Hand) — needs an external feed.
- Interactive spreadsheet filtering / host graphs (Gigasheet, A-Packets) — UI features
  with no headless page shape here.
- Live capture — out of scope (offline file parsing only).

No competitor copy, branding, or trademarks were used.

## Sources
- [A-Packets PCAP Analyzer](https://apackets.com/)
- [Red Hand Free Online PCAP Analyzer](https://redhand.io/analyzer)
- [PacketTotal](https://www.packettotal.com/)
- [ChatTCP — View and Analyze PCAP Files Online](https://chattcp.com/en/pcap-online-analysis)
- [Gigasheet Online PCAP File Viewer](https://www.gigasheet.com/popular-tools/pcap-file-viewer)

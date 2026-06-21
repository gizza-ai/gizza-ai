# strip-ipv4-header — competitor analysis (2026-06-21)

## Tool
**IPv4 Header Stripper** — paste a raw IPv4 packet as hex; the tool removes the
IPv4 header (length read from the IHL field, so IP options are stripped too),
honors the Total Length field to drop trailing link-layer padding, and returns
the encapsulated payload (the transport segment) as a lowercase hex string,
plus the payload's protocol (named) and length. Pure-Rust → runs on all
surfaces (chat / CLI / in-browser page); nothing is uploaded.

## Surfaces verified
- **Chat / LLM API** — `gizza-ai/strip-ipv4-header` block; drift-guard schema
  test passes (`packet` string param, single-sourced from the descriptor).
- **CLI** — `gizza tool strip-ipv4-header packet=<hex>` → JSON
  (`version`, `ihl`, `header_length_bytes`, `protocol`, `protocol_name`,
  `payload_length`, `payload_hex`). Verified.
- **Page** — `/tools/strip-ipv4-header/` renders a human-readable summary +
  payload hex. 2 Playwright tests pass (20-byte header path; IHL=6 options path).
- Unit tests: 12 (happy paths + IHL/options + padding-trim + 5 error cases).

## Competitors surveyed
1. **Packetor** (packetor.com) — browser hex→full multi-layer packet decoder
   (Ethernet/IP/TCP/UDP/…), Wireshark-style field tree.
2. **Hex Packet Decoder / gasmi.net** (hpd.gasmi.net) — Wireshark-powered full
   decode of a pasted hex packet, every layer dissected.
3. **Teleport TCP Header Decoder** (goteleport.com/resources/tools) — decodes a
   TCP header from hex.
4. **Wireshark / tcpdump** (desktop) — the reference dissectors; "Export Bytes"
   / follow-stream gives payloads but requires a capture and the app.
5. **Manual / scripting** (Python `struct`, scapy) — count IHL*4 bytes, slice.

## Gap analysis (fit-to-model)
The competitors all **decode the whole packet into fields** — none of them have
a dedicated "give me just the payload bytes after the IP header" output. Our
tool's distinct value is the *inverse*: decapsulation to raw payload hex you can
paste into another decoder (e.g. a TCP/UDP header parser). It is complementary
to, not a clone of, `parse-ipv4-header` (which decodes the header fields).

Closed in-model:
- **IHL-aware stripping** — options (IHL > 5) removed in full, matching how
  Wireshark advances past the header. (Verified by the options test.)
- **Total-Length-aware trimming** — drops Ethernet padding on short packets, so
  the payload matches the IP layer exactly (not the captured frame). Several
  naive "count 20 bytes" approaches leak padding; we don't.
- **Named protocol** of the carried payload (TCP/UDP/ICMP/GRE/ESP/OSPF/…) so the
  user knows which decoder to feed the bytes to next.
- **Forgiving input** — spaces, colons, dashes, dots, `0x` prefix all ignored,
  matching the paste-friendliness of the competitor tools.
- **Clear errors** — non-IPv4 version, IHL < 5, truncated header, odd/invalid
  hex are all reported with actionable messages.

## Out of model (not built — would need new infra)
- **Full multi-layer dissection** (Ethernet/VLAN/IP/TCP/UDP field trees) — that
  is a different tool (and `parse-ipv4-header` already covers the IP header
  fields); out of scope for a focused "strip" utility.
- **IPv6 / extension-header stripping** — would be a separate `strip-ipv6-header`
  tool; not in this slug's scope.
- **pcap file upload** — the page input is a hex field, not a capture parser.

No competitor copy, branding, or trademarks were used.

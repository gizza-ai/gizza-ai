## About this tool

**IPv4 Header Stripper** removes the IPv4 header from a raw packet, given as a
**hex string**, and returns the **encapsulated payload** — the transport
segment (e.g. the TCP or UDP datagram, or an ICMP message) that the IP layer
carries.

- **Honors the IHL field.** The header length is read from the Internet Header
  Length nibble, so a header with **IP options** (IHL > 5) is removed in full —
  the options never leak into the payload.
- **Honors Total Length.** When the packet's Total Length field is present and
  consistent, it is used to trim any **trailing link-layer padding** (an
  Ethernet frame pads small packets to 60 bytes), so you get exactly the IP
  payload.
- **Reports the payload protocol.** The protocol number from the header is
  shown, named when known (e.g. **TCP**, **UDP**, **ICMP**, **GRE**, **ESP**,
  **OSPF**).
- **Returns hex.** The payload is emitted as a lowercase hex string with no
  separators, ready to paste into another decoder (e.g. a TCP/UDP header
  parser).

The header is the IPv4 portion of a packet — the part before the transport
data. Stripping it leaves what the next layer sees. Input may use spaces,
colons, dashes, dots, or a leading `0x`; they are all ignored.

### Example

```
45 00 00 1c  1c 46 40 00  40 11 00 00  c0 a8 00 68  c0 a8 00 01  00 35 00 35 00 08 00 00
└──────────────────── IPv4 header (20 bytes) ─────────────────┘  └──── payload (UDP) ────┘
```

The 20-byte header is removed and the payload `0035003500080000` is returned.

### Common uses

- Pull the TCP/UDP segment out of a captured IP packet to feed a transport-layer
  decoder.
- Strip IP options and padding to isolate the exact payload bytes.
- Confirm the carried protocol and payload length by hand.

## FAQ

<details>
<summary>What happens when the packet carries IP options?</summary>

The header length is taken from the IHL nibble, not assumed to be 20 bytes. If
IHL is greater than 5, the full IHL × 4 bytes — options included — are removed,
so option bytes never end up in the extracted payload.

</details>

<details>
<summary>Why is the returned payload shorter than the bytes after the header?</summary>

When the packet's Total Length field is consistent (at least the header size and
no larger than the bytes you pasted), the payload is cut at that length. That
drops trailing link-layer padding — Ethernet pads short frames to 60 bytes. If
Total Length looks wrong, everything after the header is returned instead.

</details>

<details>
<summary>Which input formats are accepted, and what makes the tool reject a packet?</summary>

Hex with spaces, colons, dashes, dots or a leading `0x` all work — separators
are ignored. The packet is rejected with a specific error if it's under 20
bytes, its version nibble isn't 4, its IHL is below 5, or it's shorter than the
header length the IHL claims.

</details>

<details>
<summary>Can it strip an IPv6 header too?</summary>

No. The version field must be 4; an IPv6 packet (version 6, fixed 40-byte
header plus extension headers) is a different format and is rejected with a
clear message rather than mis-parsed.

</details>

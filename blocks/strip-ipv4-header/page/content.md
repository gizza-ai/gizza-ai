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

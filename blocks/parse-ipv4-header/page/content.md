## About this tool

**IPv4 Header Parser** decodes a raw IPv4 packet header, given as a **hex
string**, into every field of the 20-byte (or longer, with options) header:

- **Version & IHL** — the IP version (4) and the Internet Header Length in
  32-bit words, plus the header length in bytes.
- **DSCP & ECN** — the Differentiated Services Code Point (named when a
  well-known class such as **CS0**, **EF**, or **AF21**) and the Explicit
  Congestion Notification, both decoded from the Type-of-Service byte.
- **Total Length** — the full packet length and the implied payload length.
- **Identification & fragmentation** — the identification field, the **DF**
  (Don't Fragment) and **MF** (More Fragments) flags, and the **fragment
  offset** in both 8-byte units and bytes.
- **TTL & Protocol** — the Time To Live and the protocol number, named when
  known (e.g. **TCP**, **UDP**, **ICMP**, **GRE**, **ESP**, **OSPF**).
- **Header Checksum** — the stored checksum and whether it is **valid**,
  recomputed over the header with the one's-complement algorithm (RFC 1071).
- **Addresses** — the **source** and **destination** IPv4 addresses in dotted
  decimal.
- **Options** — any IP options bytes when IHL > 5.

The header is the IPv4 portion of a packet — the part after the link-layer
frame and before the transport (TCP/UDP) data. Input may use spaces, colons,
dashes, dots, or a leading `0x`; they are all ignored.

### Example

```
45 00 00 3c  1c 46 40 00  40 06 b1 e6  c0 a8 00 68  c0 a8 00 01
└┬┘ └┬┘ └─┬─┘ └─┬─┘ └─┬─┘ └┬┘ └┬┘ └─┬─┘ └── src ──┘ └── dst ──┘
 │   │    │     │     │   TTL  │     │
 │  ToS  total  id   flags/   proto  checksum
ver/IHL  length      frag-off
```

### Common uses

- Read an IP header captured in Wireshark/tcpdump without re-opening the capture.
- Verify a header checksum, or confirm TTL, protocol, and addresses by hand.
- Inspect fragmentation (DF/MF, offset) and QoS marking (DSCP/ECN) on a packet.

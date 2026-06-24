## About this tool

**UDP Header Parser** decodes a raw UDP datagram header (RFC 768), given as a
**hex string**, into every field of the fixed 8-byte header:

- **Ports** — the **source** and **destination** port numbers, each annotated
  with its well-known service name when recognised (e.g. **53 → DNS**,
  **123 → NTP**, **443 → QUIC/HTTP-3**, **67/68 → DHCP**, **161 → SNMP**).
- **Length** — the total datagram length in bytes. This counts the 8-byte
  header **plus** the data, so the tool also reports the implied **payload
  length** (length − 8).
- **Checksum** — the stored 16-bit checksum value, with a note when it is
  **disabled** (a checksum of `0x0000`, which UDP permits over IPv4).

A UDP header is much simpler than TCP: just four 16-bit fields and no options.
Only the first 8 bytes are read, so you can paste the whole datagram and the
trailing payload bytes are ignored. Input may use spaces, colons, dashes, dots,
or a leading `0x`; they are all ignored.

### Example

```
c3 d2  00 35  00 28  1b 6e
└─┬─┘  └─┬─┘  └─┬─┘  └─┬─┘
 src     dst   length cksum
 port    port  (40 B)
```

This decodes to source port 50130, destination port 53 (DNS), length 40 bytes
(32 bytes of payload), checksum `0x1b6e`.

### Common uses

- Read a UDP header captured in Wireshark/tcpdump without re-opening the capture.
- Confirm the ports of a DNS, NTP, DHCP, QUIC, or SNMP exchange.
- Check the datagram length and whether the UDP checksum is present or disabled.

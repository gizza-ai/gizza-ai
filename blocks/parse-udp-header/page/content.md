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

### FAQ

<details>
<summary>Can I paste the whole packet, or just the first 8 bytes?</summary>

Paste as much as you like — only the first 8 bytes are decoded, so a full datagram (or a whole hex dump line) works and the payload bytes are simply ignored. Spaces, colons, dashes, dots, and a leading `0x` are all stripped automatically.

</details>

<details>
<summary>What does "checksum disabled" mean?</summary>

A checksum field of `0x0000` means the sender skipped checksumming, which RFC 768 permits over IPv4 (over IPv6 the checksum is mandatory). The tool flags this case explicitly; note it reports the *stored* value — it can't verify the checksum, since that requires the IP pseudo-header and payload.

</details>

<details>
<summary>Which ports get a service name?</summary>

A curated set of well-known UDP services: DNS (53), DHCP (67/68), TFTP (69), NTP (123), NetBIOS (137/138), SNMP (161/162), QUIC/HTTP-3 (443), IKE (500), syslog (514), OpenVPN (1194), RADIUS (1812/1813), SSDP (1900), mDNS (5353), WireGuard (51820), and a few more. Unrecognised ports just show the number.

</details>

<details>
<summary>Why does it report both "length" and "payload length"?</summary>

The header's length field counts the 8-byte header **plus** the data, which trips people up — so the tool also shows the implied payload size (length − 8). A length below 8 is invalid and reported as such.

</details>

## About this tool

**UDP Header Stripper** removes the fixed **8-byte UDP header** from a raw UDP
datagram, given as a **hex string**, and returns the **encapsulated payload** —
the application data (e.g. a DNS message, a QUIC packet, RTP media) that UDP
carries.

- **Fixed 8-byte header.** A UDP header is always exactly eight bytes — source
  port, destination port, length, and checksum, two bytes each (RFC 768). Those
  bytes are removed and the rest is the payload.
- **Decodes the header fields.** The **source port**, **destination port**, the
  **Length** field (header + data), and the **Checksum** (where `0x0000` means
  "not computed") are shown alongside the payload.
- **Honors the Length field.** When the UDP Length field is present and
  consistent, it is used to trim any **trailing link-layer padding** (an
  Ethernet frame pads small packets to 60 bytes), so you get exactly the UDP
  payload.
- **Returns hex.** The payload is emitted as a lowercase hex string with no
  separators, ready to paste into another decoder.

The header is the UDP portion of a datagram — the part before the application
data. Stripping it leaves what the application sees. Input may use spaces,
colons, dashes, dots, or a leading `0x`; they are all ignored.

### Example

```
00 35  a1 b2  00 0c  12 34   de ad be ef
└src─┘ └dst─┘ └len┘ └csum┘   └─ payload ─┘
└──────── UDP header (8 bytes) ───────┘
```

The 8-byte header is removed and the payload `deadbeef` is returned, from a
datagram with source port 53 and destination port 41394.

### Common uses

- Pull the application payload out of a captured UDP datagram to feed a
  protocol decoder (DNS, DHCP, RTP, QUIC, …).
- Strip UDP framing and padding to isolate the exact payload bytes.
- Read the source/destination ports and length by hand from raw bytes.

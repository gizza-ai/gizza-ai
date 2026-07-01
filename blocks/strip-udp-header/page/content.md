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

## FAQ

<details>
<summary>Can I paste a whole Ethernet or IP packet?</summary>

No — the input must start at the first byte of the **UDP header**. If you paste a
full frame, the Ethernet/IP header bytes would be misread as ports and length.
From a capture, copy the bytes starting at the UDP layer (in Wireshark:
right-click the UDP layer → Copy → "…as a Hex Stream").

</details>

<details>
<summary>Why is the payload shorter than the bytes I pasted?</summary>

The UDP **Length** field (header + data) is honored when it's consistent with the
input: anything beyond it is treated as trailing link-layer padding and dropped.
Ethernet pads small frames to 60 bytes, so captured datagrams often carry a few
padding bytes that are not part of the payload.

</details>

<details>
<summary>What does a checksum of 0x0000 mean?</summary>

That the sender didn't compute one. Over IPv4 the UDP checksum is optional and
`0x0000` explicitly means "not computed" (RFC 768) — the tool reports it as such
rather than flagging an error.

</details>

<details>
<summary>What hex formats are accepted?</summary>

Pretty much anything a capture tool exports: spaces, colons, dashes, dots, and a
leading `0x` are all ignored, so `00 35 a1 b2 …`, `00:35:a1:b2:…` and
`0x0035a1b2…` parse identically. The datagram must be at least 8 bytes (the fixed
UDP header size).

</details>

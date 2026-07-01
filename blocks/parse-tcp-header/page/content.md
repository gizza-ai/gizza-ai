## About this tool

**TCP Header Parser** decodes a raw TCP segment header, given as a **hex
string**, into every field of the 20-byte (or longer, with options) header:

- **Ports** — the **source** and **destination** port numbers.
- **Sequence & Acknowledgement** — the 32-bit sequence number and the
  acknowledgement number (significant only when the **ACK** flag is set), in
  both decimal and hex.
- **Data Offset** — the header length in 32-bit words and in bytes (a plain
  header is 5 words / 20 bytes; options extend it up to 60 bytes).
- **Flags** — the nine control bits **NS**, **CWR**, **ECE**, **URG**, **ACK**,
  **PSH**, **RST**, **SYN**, and **FIN**, plus a compact list of the set flags
  (e.g. `SYN ACK`).
- **Window** — the advertised receive window size.
- **Checksum** — the stored 16-bit checksum value.
- **Urgent Pointer** — significant only when the **URG** flag is set.
- **Options** — any TCP options when the data offset > 5, parsed into named
  entries (**MSS**, **Window Scale**, **SACK-Permitted**, **SACK**,
  **Timestamps**, **NOP**, **End of Option List**) with their length and a
  decoded value for the fixed-shape options.

The header is the TCP portion of a segment — the part after the IP header and
before the application payload. Input may use spaces, colons, dashes, dots, or a
leading `0x`; they are all ignored.

### Example

```
c2 13  00 50  4f 8e 6b 1a  00 00 00 00  50 02  ff ff  fe 34  00 00
└─┬─┘  └─┬─┘  └────┬────┘  └────┬────┘   ┬     └─┬─┘  └─┬─┘  └─┬─┘
 src     dst      seq          ack    offset  window cksum  urg
 port    port                        + flags
```

### Common uses

- Read a TCP header captured in Wireshark/tcpdump without re-opening the capture.
- Confirm the ports, flags (SYN/ACK/FIN), sequence, and window of a handshake.
- Inspect TCP options such as MSS and Window Scale negotiated on a SYN.

## FAQ

<details>
<summary>Can I paste bytes copied straight from Wireshark or tcpdump?</summary>

Yes — the hex may contain spaces, colons, dashes, dots, or a leading `0x`;
all separators are stripped before decoding. Just make sure you copy the
**TCP** portion of the frame (after the Ethernet and IP headers), because the
parser reads byte 0 as the start of the source port.

</details>

<details>
<summary>Why do I get a "data offset says the header is N bytes" error?</summary>

The 4-bit data-offset field claims the header is longer than the hex you
pasted. A plain header is 5 words (20 bytes); with options it grows up to
60 bytes. Either you truncated the copy, or you pasted something that isn't
the start of a TCP header — a data offset below 5 is rejected outright too.

</details>

<details>
<summary>Is it a problem if I include payload bytes after the header?</summary>

No. The parser reads exactly the number of bytes the data offset declares
(20–60) and ignores anything after them, so pasting a whole segment — header
plus application data — decodes fine.

</details>

<details>
<summary>Does the tool verify the TCP checksum?</summary>

No — it reports the stored 16-bit checksum value but can't validate it,
because the TCP checksum is computed over a pseudo-header that includes the
source and destination **IP addresses**, which aren't part of the TCP header
you paste.

</details>

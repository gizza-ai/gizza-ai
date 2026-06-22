## About this tool

**Ethernet Frame Parser** decodes a raw Ethernet II or IEEE 802.3 frame, given
as a **hex string**, into its header fields:

- **MAC addresses** — destination and source, each flagged as
  **broadcast**, **multicast**, or **unicast**, and the source flagged
  **globally unique (OUI)** vs **locally administered**.
- **VLAN tags** — any 802.1Q (`0x8100`) or 802.1ad (`0x88a8`) tags, including
  stacked **Q-in-Q**, decoded to **TPID**, **PCP** (priority), **DEI**, and
  **VID** (VLAN id).
- **Type / length** — the type field decoded as an **EtherType** (named when
  known, e.g. IPv4, ARP, IPv6, LLDP, MPLS, PPPoE) when ≥ `0x0600`, or as an
  **802.3 length** when ≤ `1500`.
- **Payload** — the remaining bytes (length + hex).

The frame is everything **after** the preamble/SFD and **before** the FCS — the
14-byte header (6 dst + 6 src + 2 type) plus the payload. Input may use spaces,
colons, dashes, or a leading `0x`; they are all ignored.

### Example

```
ff ff ff ff ff ff  00 11 22 33 44 55  08 06  00 01 08 00 ...
└── destination ──┘ └──── source ────┘ └type┘ └─ ARP payload ─┘
```

### Common uses

- Read a frame captured in Wireshark/tcpdump without re-opening the capture.
- Confirm VLAN tagging (single tag vs Q-in-Q) and the VID/priority on a trunk.
- Look up an unfamiliar EtherType, or tell an Ethernet II frame from 802.3.

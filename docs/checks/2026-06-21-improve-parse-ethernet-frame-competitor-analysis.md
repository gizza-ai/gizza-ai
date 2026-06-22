# parse-ethernet-frame — competitor analysis (2026-06-21)

Tool: decode an Ethernet II / IEEE 802.3 frame from a hex string into destination/source
MAC, broadcast/multicast & U/L flags, 802.1Q / 802.1ad VLAN tags (incl. Q-in-Q), EtherType
(named) or 802.3 length, and the payload. Pure-Rust → runs on chat, CLI, and the page.

## Surfaces verified
- **Chat block:** `wafer build` validates + instantiates `target/block.wasm` (307.8 KiB). Returns JSON.
- **CLI:** `gizza tool parse-ethernet-frame frame="aabbccddeeff112233445566 8100 200a 0800 deadbeef"` → correct JSON (VLAN VID=10, EtherType IPv4, payload deadbeef). Exit 0.
- **Page:** `/tools/parse-ethernet-frame/` Playwright 2/2 pass (VLAN/IPv4 frame + broadcast ARP frame).
- **Unit tests:** 13 core tests + 1 drift-guard schema test pass.

## Competitors surveyed
1. **Hex Packet Decoder (hpd.gasmi.net)** — Wireshark-style multi-layer decode. Shows dst/src MAC with **OUI vendor name**, EtherType, and the I/G + U/L bit interpretation. Decodes deeper L3/L4 layers (IPv6/TCP/…). No explicit VLAN tag breakdown shown in its default sample.
2. **Packetor (packetor.com)** — graphical multi-layer packet view; lets you pick the L2 protocol; full nested-protocol decode.
3. **CalculateYogi Hex Packet Decoder** — Ethernet/IPv4/TCP/UDP/ICMP/ARP from hex.
4. **CyberChef** — general-purpose; has packet/hex operations but no dedicated Ethernet-frame view.
5. **EthernetFrameParser (GitHub, C CLI)** — decodes Ethernet II into dst/src MAC, EtherType, payload. CLI only, no VLAN.

## Gap analysis & actions
| Capability | Competitors | This tool | Action |
|---|---|---|---|
| dst/src MAC parse | all | yes | covered |
| I/G (multicast) + broadcast flag | HPD | yes | covered |
| U/L (locally administered) flag | HPD | yes | covered |
| EtherType + human name | all | yes (45+ named types) | covered |
| Ethernet II vs 802.3 length disambiguation | partial | yes (0x0600 threshold) | covered |
| 802.1Q VLAN tag (PCP/DEI/VID) | partial | **yes** | exceeds most |
| 802.1ad / Q-in-Q stacked tags | rare | **yes** | exceeds most |
| Payload length + hex | most | yes | covered |
| Lenient input (spaces/colons/dashes/0x) | partial | yes | covered |
| **OUI → vendor name lookup** | HPD | no | **out of model** — needs the full IEEE OUI database (~30k entries, multi-MB); too large to embed in a pure wasm block. Documented as a non-goal. |
| **Deeper L3/L4 decode (IP/TCP/UDP)** | HPD, Packetor | no | **out of scope** — this is a focused L2 frame parser; separate tools would handle higher layers. |

## Conclusion
The tool matches or exceeds the in-scope L2 feature set of the surveyed decoders (notably
including full 802.1Q + Q-in-Q VLAN decoding, which several competitors omit). The two
competitor features not implemented (OUI vendor lookup, deep L3/L4 decode) are out of the
pure-wasm model / out of scope and are documented as non-goals. No competitor copy, branding,
or trademark was reused.

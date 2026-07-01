## What this tool does

Every network interface — Wi-Fi, Ethernet, Bluetooth — has a **MAC address**, and
its first three octets (the **OUI**, Organizationally Unique Identifier) are
assigned to a specific manufacturer by the IEEE. Paste a MAC address (or just its
OUI) and this tool tells you which company made the device.

The full IEEE OUI registry is bundled into the page, so the lookup runs
**entirely in your browser** — nothing is sent to a server, it works offline, and
there is no sign-up.

## Accepted formats

You can paste a MAC address in any of the common forms, in upper or lower case,
and either the full address or just the first three octets:

| Form | Example |
| --- | --- |
| Colon | `28:6F:B9:01:23:45` |
| Hyphen | `28-6F-B9-01-23-45` |
| Dot (Cisco) | `286f.b901.2345` |
| Bare hex | `286fb9012345` |
| OUI only | `28:6F:B9` |

EUI-64 (8-octet) addresses are accepted too — only the first three octets are
needed for the vendor.

**Batch lookup:** paste several addresses, one per line, to resolve them all at
once — you'll get one compact `MAC — Vendor` line per address (great for an ARP
table or a device inventory).

## What you get back

- **MAC** — your address normalized to canonical colon-separated uppercase.
- **OUI** — the 24-bit prefix used for the lookup.
- **Vendor** — the registered organization name from the IEEE registry, or a note
  that the OUI is unassigned (common for randomized / locally-administered MACs).
- **Type** — decoded from the first octet's two special bits:
  - *globally unique* (IEEE-assigned) vs *locally administered* (randomized or
    manually set — these are not in the registry by design);
  - *unicast* vs *multicast / group* address.

## FAQ

<details>
<summary>Why is my phone's Wi-Fi MAC not found?</summary>

Modern phones use *MAC randomization*
for privacy. A randomized MAC has the locally-administered bit set and is not an
IEEE assignment, so there is no vendor to look up — the tool will say so.

</details>

<details>
<summary>Is it free and private?</summary>

Yes. The registry is bundled into the page, so your
input never leaves your device and it keeps working offline once loaded.

</details>

<details>
<summary>What's the difference between an OUI and a MAC address?</summary>

The OUI is the first
24 bits (3 octets) of a 48-bit MAC address and identifies the manufacturer; the
remaining bits identify the individual device. Only the OUI matters for a vendor
lookup.

</details>

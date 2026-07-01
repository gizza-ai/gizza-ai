## About this tool

**Extract MAC Addresses** scans pasted text or a log file and pulls out every
**MAC address** it contains — written in any common notation — then normalizes
them all to the single format you choose and deduplicates them.

- **Every notation**: colon (`00:1A:2B:3C:4D:5E`), hyphen
  (`00-1A-2B-3C-4D-5E`), Cisco dotted-quad (`001a.2b3c.4d5e`), and bare hex
  (`001A2B3C4D5E`) are all recognized — including 64-bit **EUI-64** addresses.
- **Normalized output**: pick colon, hyphen, Cisco, or bare and every address
  is rewritten that way, so a mixed-notation paste becomes a clean uniform list.
- **Deduplicated** by the underlying bytes, in first-seen order — the same
  address written two different ways counts once.
- **Robust to noise**: a 32-character hash or a longer hex blob is ignored, so
  you don't get false positives from MD5/SHA hex.

Everything runs **locally in your browser** via WebAssembly — your logs are
never uploaded.

### Handy for

- Pulling NIC / device MACs out of DHCP leases, ARP tables, or switch logs.
- Converting a list of MACs from Cisco dotted-quad to colon form (or vice versa).
- Building a unique device inventory from a paste of mixed network output.

## FAQ

<details>
<summary>Won't MD5 hashes or other hex strings show up as false matches?</summary>

No. A bare hex run only counts as a MAC when it's exactly 12 hex digits (EUI-48) or
16 (EUI-64) — a 32-character MD5, a 40-character SHA-1, or any longer hex blob is
skipped. Separator forms (colons, hyphens, Cisco dots) are matched by their exact
structure, so ordinary log noise doesn't leak in.

</details>

<details>
<summary>The same MAC appears twice in my log in different notations — why is it listed once?</summary>

Deduplication works on the underlying bytes, not the spelling. `00:1A:2B:3C:4D:5E`,
`001a.2b3c.4d5e`, and `001A2B3C4D5E` are all the same address, so it appears once,
at the position it was first seen.

</details>

<details>
<summary>Can I get the output in uppercase?</summary>

Not currently — every address is normalized to lowercase hex in your chosen
notation (`00:1a:2b:3c:4d:5e` colon form, `00-1a-2b-3c-4d-5e` hyphen,
`001a.2b3c.4d5e` Cisco, `001a2b3c4d5e` bare). If a downstream system needs
uppercase, run the list through a case converter afterwards.

</details>

<details>
<summary>Does it handle 64-bit EUI-64 addresses?</summary>

Yes. 16-hex-digit EUI-64 identifiers (used in IPv6 interface IDs, Zigbee, and
firewire) are recognized in all four notations and reformatted alongside the
ordinary 48-bit MACs.

</details>

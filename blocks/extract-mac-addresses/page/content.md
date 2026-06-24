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

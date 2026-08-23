## About this tool

Use this when you need to check whether IP addresses are syntactically valid before they go into a firewall rule, allowlist, log pipeline, DNS zone or spreadsheet. Paste one address per line and the tool reports the canonical spelling, family, category and a specific reason for any bad line.

IPv4 and IPv6 are both supported. IPv6 output defaults to the RFC 5952 compressed form, so `2001:0DB8:0000:0000:0000:0000:0000:0001` becomes `2001:db8::1`; switch to **Expanded** when you need all eight groups. Optional `/prefix`, `:port`, bracketed IPv6 authority syntax and `%zone` ids can be allowed or refused independently.

Classification is syntax-only and local: private, loopback, link-local, multicast, documentation, CGNAT/shared, unique-local, mapped IPv4, 6to4, Teredo, NAT64, benchmarking, broadcast, reserved and global addresses are labeled from their numeric ranges. Nothing is pinged, geolocated or sent over the network.

Choose **Report** for a readable audit, **Table** for CSV, **JSON** for automation, **Valid only** for a canonical list to paste elsewhere, **Invalid only** for cleanup, or **Summary** for totals by family/category. **Drop duplicates** compares canonical forms, so different IPv6 spellings of the same address collapse together.

Limits and edge cases:

- Up to 5,000 non-blank lines or 500,000 bytes per run.
- Leading-zero IPv4 octets are rejected by default because some legacy parsers treat them as octal; enable the checkbox only for decimal input you trust.
- Classification is based on address ranges, not reachability. A syntactically global address might still be blocked, unused or unrouted in the real network.
- Ports are validated as numbers in the range 0-65535 but the tool does not test whether anything is listening there.

## FAQ

<details>
<summary>Does this check whether an address is reachable?</summary>

No. It validates syntax, canonicalizes the text and classifies the address range. It never pings, opens a socket, performs DNS, geolocates, or contacts a remote service.

</details>

<details>
<summary>Why are IPv4 leading zeros rejected?</summary>

They are ambiguous. Modern decimal-only parsers may read `192.168.001.010` as `192.168.1.10`, while old C-derived parsers can interpret leading-zero octets as octal. The default rejects them; turn on **Allow IPv4 leading zeros** only when you know the source means decimal.

</details>

<details>
<summary>Can I validate CIDR prefixes and ports?</summary>

Yes. `10.0.0.0/8`, `2001:db8::/32`, `192.0.2.7:443` and `[2001:db8::1]:443` are accepted by default and checked for family-specific prefix/port bounds. Disable the prefix or port checkbox when a field must contain a bare address.

</details>

<details>
<summary>What is the difference between compressed and expanded IPv6?</summary>

Compressed is the normal RFC 5952 style: lowercase hex, no leading zeroes, and the longest zero run collapsed to `::`. Expanded writes all eight 16-bit groups with four hex digits each. Both identify the same address.

</details>

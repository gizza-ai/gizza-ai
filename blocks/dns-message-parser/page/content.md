## About this tool

**DNS Message Parser** decodes a raw **DNS protocol message** (the RFC 1035 wire
format) — given as a **hex string** or **base64url** — into its header, question,
and resource-record sections.

Every DNS query and response is a single message that starts with a fixed
**12-byte header**, followed by four counted sections.

### The header

- **ID** — the 16-bit transaction id that pairs a response with its query.
- **QR** — whether the message is a **query** or a **response**.
- **Opcode** — `QUERY`, `IQUERY`, `STATUS`, `NOTIFY`, or `UPDATE`.
- **Flags** — **AA** (authoritative answer), **TC** (truncated), **RD**
  (recursion desired), **RA** (recursion available), **AD** (authentic data) and
  **CD** (checking disabled) for DNSSEC.
- **RCODE** — the result code: `NOERROR`, `FORMERR`, `SERVFAIL`, `NXDOMAIN`,
  `REFUSED`, and more.
- **Counts** — how many entries are in the question (QD), answer (AN), authority
  (NS), and additional (AR) sections.

### Records

The **question** names what was asked (a domain name, a QTYPE such as `A` or
`MX`, and a QCLASS, usually `IN`). The **answer**, **authority**, and
**additional** sections carry resource records. Domain names are
**decompressed** — the message-compression pointers (`0xC0…`) that DNS uses to
avoid repeating a suffix are followed back to their target, so you always see the
full name.

Common record types are decoded to readable values:

- **A** / **AAAA** — IPv4 / IPv6 addresses (IPv6 shown in compressed `::` form).
- **NS** / **CNAME** / **PTR** / **DNAME** — a domain name.
- **MX** — mail-server preference and exchange.
- **TXT** / **SPF** — the quoted text strings.
- **SOA** — primary server, responsible mailbox, serial, and the timers.
- **SRV** — priority, weight, port, and target.
- **CAA** — certificate-authority authorization flags, tag, and value.
- **OPT / EDNS0** — the advertised UDP payload size, EDNS version, and DNSSEC OK
  bit.

Any unrecognized type falls back to a hex dump of its RDATA so nothing is hidden.

### Where to get the bytes

A DNS message is the payload of a UDP/TCP packet (or, for **DNS-over-HTTPS**, the
body or the base64url `?dns=` parameter). In Wireshark, expand the **Domain Name
System** layer and copy the bytes (right-click → *Copy → …as a Hex Stream*), or
take them from a `tcpdump` capture or a DoH request. Hex input may use spaces,
colons, dashes, dots, commas, or a leading `0x`; base64url is auto-detected.

### Common uses

- Decode a captured **response** to read the answers, TTLs, and the RCODE without
  re-opening the capture.
- Inspect a **DNS-over-HTTPS** `?dns=` value to see exactly what was asked.
- Confirm the **flags** (RD/RA/AA) and **EDNS0** options a resolver negotiated.

## FAQ

<details>
<summary>What input formats does the parser accept?</summary>

Hex or base64url, auto-detected. Hex may be separated by spaces, colons, dashes,
dots, or commas and may carry a leading `0x` — so a Wireshark "Copy as Hex Stream"
or a colon-separated dump both paste straight in. Base64url is the exact form used
in a DNS-over-HTTPS GET `?dns=` parameter.

</details>

<details>
<summary>What if a record type isn't recognized?</summary>

Well-known types (A, AAAA, NS, CNAME, PTR, DNAME, MX, TXT, SPF, SOA, SRV, CAA,
OPT/EDNS0) are decoded to readable values. Any other type falls back to a raw hex
dump of its RDATA, so unusual or private-use records are still fully visible
instead of being dropped.

</details>

<details>
<summary>How are truncated or corrupt messages handled?</summary>

The parser reports a precise error — e.g. a name or RDATA that "runs past end of
message", an invalid hex digit, or a compression pointer that targets beyond the
message. Compression-pointer loops are detected by capping the label count, so a
malicious message can't hang the tool.

</details>

<details>
<summary>Does it follow DNS name compression?</summary>

Yes. The `0xC0…` compression pointers that DNS uses to avoid repeating a domain
suffix are followed back to their target, so every name in the output is shown in
full rather than as a pointer offset.

</details>

# dns-message-parser — competitor analysis (2026-06-23)

Snapshot for the new `dns-message-parser` tool: decode a DNS wire-format message
(RFC 1035), given as hex or base64url, into the header, question, and the
answer/authority/additional resource records, with domain-name decompression.

## Surfaces verified (Phase 1)

- **Chat / LLM API:** `cargo test --workspace` green (8 core + 1 drift-guard). The
  drift-guard test pins the authored chat schema to `descriptor()`; `wafer build`
  validates the chat `block.wasm` (328 KiB, instantiates).
- **CLI:** `gizza tool dns-message-parser message="1234 8180 0001 0001 ..."` →
  correct JSON (header flags, question `example.com A IN`, A answer
  `93.184.216.34` via a 0xC00C compression pointer).
- **Page (query-params):** Playwright `tool-page-dns-message-parser.spec.ts` — 3
  tests green: hex response decode, base64url (DoH form) decode, and the
  `?message=` deep-link.

## Competitors surveyed (paraphrased — no copy reused)

1. **Hex Packet Decoder (hpd.gasmi.net)** — decodes a full packet hex dump
   (Ethernet/IP/UDP/DNS) using a Wireshark-style engine. Strength: whole-stack
   decode + a field tree. Out of scope for a single DNS-layer tool (gizza already
   ships separate `parse-ipv4-header` / `parse-udp-header` / `parse-tcp-header` /
   `parse-ethernet-frame` tools for the lower layers).
2. **Packetor (packetor.com)** — browser front-end over the `tshark` engine; paste
   a hex dump, get a Wireshark-like dissection of every encapsulated protocol.
   Same whole-stack angle; depends on a server-side tshark.
3. **dnslib (Python)** — encode/decode DNS wire format; rich RR-type coverage and a
   zone-file-style text representation. A library, not a paste-and-go web tool.
4. **dns-packet (npm, mafintosh)** — abstract-encoding encode/decode of DNS
   packets; broad RR coverage, used in DoH clients. Library, JSON-shaped output.
5. **"Implement DNS in a weekend" (Julia Evans)** — a teaching reference for the
   wire format / name compression; not a tool, but the canonical spec walkthrough
   used to cross-check the header bit layout and the 0xC0 pointer handling.

## Gap analysis (fit-to-model)

gizza tools are browser-local wasm, no account, no server. Against that filter:

**In-model — covered at launch (no gap to close):**
- 12-byte header fully decoded: id, QR, opcode (named), AA/TC/RD/RA/Z/AD/CD flags,
  RCODE (named), and the QD/AN/NS/AR counts — matches/exceeds the dissectors.
- All four sections (question + answer/authority/additional), each RR with
  name/type/class/ttl/rdlength.
- **Name decompression** — the 0xC0 pointers are followed (with a loop guard),
  including pointers inside RDATA (NS/CNAME/MX/SOA/SRV targets). This is the
  feature casual hex viewers miss.
- Per-type RDATA decode: A, AAAA (with `::` zero-run compression), NS, CNAME, PTR,
  DNAME, MX, TXT/SPF (quoted chunks), SOA (all 7 fields), SRV, CAA, and **OPT /
  EDNS0** (UDP payload size, version, DO bit). Unknown types fall back to a hex
  dump so nothing is hidden.
- **Two input encodings**: hex (separators/0x tolerated) AND **base64url** — the
  exact form DNS-over-HTTPS puts in the GET `?dns=` parameter. None of the simple
  hex viewers accept the DoH base64url form directly; this is our differentiator.
- Both a JSON surface (chat/CLI) and a human-readable render (page), and graceful
  `notes` on truncated/trailing bytes rather than a hard failure.

**Out-of-model (considered, deliberately not built):**
- Full lower-layer (Ethernet/IP/UDP) dissection like hpd/packetor — out of scope
  for a DNS-*message* tool; gizza ships dedicated tools for those layers already,
  and packetor's value comes from a server-side tshark we don't run.
- DNSSEC RR *cryptographic* decode (RRSIG/DNSKEY/DS validation) — we name the
  types and hex-dump their RDATA; full signature verification is a much larger,
  separate concern.
- Encoding a message *from* fields (the reverse direction) — a distinct tool.

## Conclusion

The launch implementation already covers every in-model capability the
library/dissector competitors expose for a DNS-message-layer tool, plus the
base64url/DoH input that the simple hex viewers lack. No additional in-model gap
to close this round.

## Sources

- [Hex Packet Decoder](https://hpd.gasmi.net/)
- [Packetor](https://packetor.com/)
- [dnslib](https://github.com/andreif/dnslib)
- [dns-packet](https://github.com/mafintosh/dns-packet)
- [Implement DNS in a weekend — Part 2: Parse the response](https://implement-dns.wizardzines.com/book/part_2)

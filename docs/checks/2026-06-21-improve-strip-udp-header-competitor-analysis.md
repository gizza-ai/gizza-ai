# strip-udp-header — competitor analysis (2026-06-21)

## Tool
Removes the fixed 8-byte UDP header (RFC 768) from a raw UDP datagram supplied
as a hex string and returns the encapsulated application payload as hex, plus
the decoded header fields (source port, destination port, length, checksum).
Honors the UDP Length field to drop trailing link-layer padding. Pure-Rust → runs
on all surfaces (chat block, CLI, in-browser page).

## Surfaces verified
- **Chat/LLM API:** `wafer build` validated `target/block.wasm` (294.5 KiB);
  drift-guard schema test (`schema_json_matches_authored_chat_schema`) passes.
- **CLI:** `gizza tool strip-udp-header datagram="0035 a1b2 000c 1234 deadbeef"`
  returns the decoded JSON; padding case drops trailing bytes correctly.
- **Page:** Playwright `tool-page-strip-udp-header.spec.ts` — 2/2 pass (payload +
  field decode; Length-field padding drop).

## Competitor landscape
No tool is a dedicated "strip UDP header" utility; the capability appears as a
sub-feature inside broader packet decoders. Surveyed:

1. **Wireshark / tshark (UDP dissector).** The reference desktop decoder. Parses
   the full UDP header and shows the payload as a sub-tree. Heavyweight, requires
   a capture; not a paste-hex-get-payload web tool. *Out of scope to match
   (full capture analyzer).*
2. **CyberChef ("From Hex" + manual byte slicing).** General-purpose; a user can
   hex-decode and drop 8 bytes by hand, but there is no UDP-aware recipe — no
   field labels, no Length-aware trimming. We are more direct and structured.
3. **scapy (`UDP(bytes.fromhex(...))`).** Python REPL; decodes fields + payload.
   Requires a local Python env. We give the same field decode with zero install
   in the browser/CLI/chat.
4. **hpd / online "packet header parser" sites (e.g. hex packet decoders).**
   Decode an entire IP+UDP frame. Most assume a full IP packet, not a bare UDP
   datagram, and don't isolate just the payload as copy-ready hex.
5. **Our own `strip-ipv4-header`.** Complementary, not a duplicate: it removes the
   IP header to yield the transport segment (the UDP datagram); this tool removes
   the next layer (the UDP header) to yield the application payload. They chain.

## Gap analysis (fit to model)
- **Capability parity:** covered. Field decode (ports/length/checksum), 8-byte
  header removal, payload-as-hex, Length-field padding trim, lenient input
  (spaces/colons/dashes/dots/`0x`), header-only (empty payload) handling, and
  graceful fallback when the Length field is bogus.
- **Copy/UX:** page shows all four header fields + a labeled ASCII diagram in the
  About copy; checksum `0x0000` is annotated "not computed". Multiline input so
  pasted wrapped hex dumps are preserved.
- **In-model gaps closed:** decoding the header fields (not just dropping 8 bytes)
  and honoring the Length field for padding — both implemented.
- **Out-of-model (not built, by design):** parsing from a live `.pcap` capture,
  IP-pseudo-header checksum *validation* (needs the IP src/dst, which a bare UDP
  datagram doesn't carry), and protocol-specific payload dissection (DNS/RTP/QUIC)
  — these belong to dedicated decoders, not a header-stripper.

## Verdict
Single-purpose, in-model, distinct from existing blocks. No competitor copy or
branding used. Shipped with all three surfaces verified.

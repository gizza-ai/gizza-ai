# pcap-grep — competitor analysis (2026-07-28)

Tool: **pcap-grep** — search packet payloads in an uploaded `.pcap`/`.pcapng` with a regular
expression and return the matching packets with their metadata, ngrep-style. Browser-local /
wasm / no-account. All copy below is paraphrased — no competitor text, branding, or trademarks
copied.

## Competitors scanned (top real tools)

1. **ngrep** (`jpr5/ngrep`, "network grep") — the canonical CLI: matches an extended regular OR
   hexadecimal expression against packet *data payloads*, understands IPv4/6, TCP, UDP, ICMPv4/6,
   IGMP and Raw. Reads from a live interface or a `-I file.pcap`.
2. **tshark / Wireshark display filters** — `-Y 'frame matches "regex"'` and
   `tcp/udp contains`/`matches` operators run a regex/byte match over frame/payload bytes and
   print the matching frames with a metadata summary line.
3. **A-Packets / online pcap analyzers** — browser upload of a `.pcap`, per-packet decode and a
   payload/keyword search box over the reconstructed streams.

## Table-stakes params / behaviours (each tagged in-model / out-of-model)

| Capability | ngrep flag | Decision |
| --- | --- | --- |
| Regex over packet payload | pattern arg | **in-model** — `pattern` (required), `regex::bytes` engine |
| Case-insensitive match | `-i` | **in-model** — `ignore_case` bool |
| Hexadecimal match expression | `-X` | **in-model** — `hex` bool (pattern is a hex byte string) |
| Invert match (show non-matching) | `-v` | **in-model** — `invert` bool |
| Stop after N matches | `-n num` | **in-model** — `limit` (max matches returned) |
| Hex + ASCII payload dump | `-x` | **in-model** — `show_hex` bool (canonical hex/ascii dump) |
| Per-packet metadata line (ts, src→dst, proto, ports, TCP flags) | default output | **in-model** — every match carries this |
| ASCII-safe payload rendering (non-printables as `.`) | default output | **in-model** — `payload_ascii` |
| Port narrowing (a poor-man's BPF host/port) | BPF expr | **in-model (subset)** — optional `port` filter (src or dst) |
| Full BPF filter language (`host x and port y and ...`) | BPF expr | **out-of-model** — needs a libpcap BPF compiler/VM; a single `port` narrow covers the common case |
| Live interface capture (`-d eth0`) | `-d` | **out-of-model** — no raw-socket access in a browser sandbox; upload a capture instead |
| Save matches back to a `.pcap` (`-O`) | `-O` | **out-of-model** — output is a structured match report, not a re-muxed capture |
| Protocol dissection / stream reassembly | Wireshark | **out-of-model** — pcap-grep is a payload grep, not a full dissector (see the sibling `parse-pcap` / `pcap-network-forensics` tools) |

## UX control patterns competitors ship

- ngrep/tshark are CLI-only (flags). Our surfaces are **chat + CLI** (no page: a file-in →
  text-report tool fits neither the pure-text page nor the ffmpeg file→media page shape — same
  no-page pattern as `parse-pcap`, `pcap-network-forensics`, `detect-file-type`). So the
  "control pattern" is descriptor params with good `.describe()` text + booleans, exercised from
  chat and the CLI.

## Design decisions

- Search the **application payload** (bytes after the TCP/UDP header; after the IP header for
  other IP protocols; L2 payload for non-IP) — this is what ngrep matches, not header bytes.
- `regex::bytes::RegexBuilder` with `unicode(false)` so arbitrary payload bytes (and `\xHH`
  escapes) match; `case_insensitive` wired to `ignore_case`.
- Hex mode converts the hex string (spaces/colons tolerated) into a `\xHH…` byte pattern so the
  same engine reports match offsets.
- Caps: `limit` matches returned (total match count still reported → `truncated`); payload
  rendering and the optional hex dump are byte-capped to keep the report readable, with the cap
  stated in the output and on error messages.

Every table-stake above ends in the descriptor or the out-of-model list — none dropped silently.

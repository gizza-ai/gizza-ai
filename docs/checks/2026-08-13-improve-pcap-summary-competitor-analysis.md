# pcap-summary — competitor analysis (2026-08-13)

Backlog row: `pcap-summary` — "Summarizes an uploaded pcap: protocol breakdown, top
talkers, conversations, and busiest ports." type_hint `pure`.

Scan run BEFORE implementing. Everything below is paraphrased from public
documentation and product pages; no competitor copy, branding, or trademarked
wording is reproduced, and no competitor UI text is used in the descriptor or in
`manifest.json`.

## Scope of the backlog row

Four named deliverables: **protocol breakdown**, **top talkers**,
**conversations**, **busiest ports** — the "capture statistics" lens, i.e. the
question "what is in this capture and who is doing the talking", answered from a
single uploaded file with no live capture and no decryption.

## Duplicate / viability check (done first)

`ls blocks/ | grep -iE 'pcap|network|packet|capture'` → four existing pcap blocks.
Each was read (`core/src/lib.rs` + `src/lib.rs`) before deciding:

| Existing block | What it does | Overlap with this row |
| --- | --- | --- |
| `parse-pcap` | per-packet decode → one row per packet (index, timestamp, src/dst, ports, protocol, info) | none of the four aggregates; it is the packet dump, capped by `limit` |
| `pcap-grep` | ngrep-style regex/hex search over packet payloads | none — payload search, not statistics |
| `pcap-file-extractor` | TCP reassembly → carve HTTP/FTP/SMB objects out of the capture | none — object carving |
| `pcap-network-forensics` | forensic findings: host inventory, conversations, DNS questions, HTTP request lines, cleartext credentials | **partial** — `hosts` and `conversations` |

Verdict: **not a duplicate, build it.** The row is not a subset of any existing
block. `pcap-network-forensics` is the closest sibling and covers two of the four
deliverables, but:

* it has **no protocol breakdown** — no per-protocol or per-layer packet/byte
  aggregation anywhere in `Report` (fields are `hosts`, `conversations`,
  `dns_queries`, `http_requests`, `credentials`);
* it has **no port ranking** — ports appear only inside conversation endpoint
  strings (`ip:port`), never aggregated or ranked;
* its `hosts`/`conversations` are **non-directional** (`packets` + `bytes` only),
  so "who sent vs who received" — the actual top-talker question — is not
  answerable from its output;
* it carries no capture-level statistics (average packet size, packet/bit rate,
  snaplen, truncation).

The two blocks are also different lenses on purpose: that one answers "what
suspicious artefacts are in here" (credentials, DNS, HTTP), this one answers
"what does this capture consist of" (rates, protocol mix, rankings). Emitting
talkers and conversations here is unavoidable — they are two of the four named
deliverables, and a summary that omitted them would not satisfy the row.

Viability confirmed live rather than assumed: file-input url fetch works on this
box today —
`gizza tool pcap-network-forensics url=https://raw.githubusercontent.com/the-tcpdump-group/tcpdump/master/tests/dhcp-rfc3004.pcap section=hosts`
returned a real report, so the `Input::File` url⊕ref surface is CLI-verifiable
(unlike the 2026-07-18 → 2026-07-25 window when `wafer-run/network` was broken
and file-input rows were skiplisted for it).

## Competitors reviewed

Three real tools, chosen because each is a genuine implementation of this exact
summary, not a listicle.

### 1. Wireshark / tshark — Statistics + capinfos

The reference implementation, and the only one whose field list is fully
documented. Three separate features together equal this row:

* `-z io,phs` — protocol hierarchy statistics: packet **and** byte counts per
  protocol, presented as a layer tree; accepts an optional display filter and may
  be given more than once.
* `-z conv,TYPE` — conversation table: frames and bytes **per direction** plus
  totals, relative start time, and duration; sorted by total frame count.
  `TYPE` selects the address family (`eth`, `ip`, `ipv6`, `tcp`, `udp`, `sctp`,
  `wlan`, and many link-specific ones).
* `-z endpoints,TYPE` — endpoint table: total packets and bytes plus per-direction
  counts, sorted by total packets. Same `TYPE` set.
* `capinfos` — capture-file properties: file type, encapsulation, snaplen, packet
  count, total data size, first/last packet time, duration, average packet size,
  average packet rate, average byte rate, average bit rate, strict-time-order
  flag, file hashes.

Takeaways: byte counts must sit beside packet counts everywhere; ranked tables
need an explicit sort column; direction matters; the capture-level header block is
a first-class section, not decoration.

### 2. A-Packets (browser upload, free tier)

Upload a `.pcap`/`.pcapng` and get a report without installing anything. Reports
host relationships and per-host services, HTTP sessions, wireless artefacts,
plaintext credentials, extracted files, and flags patterns such as scans. Free
uploads are capped around 25 MB. Results default to a publicly viewable page,
with private and on-prem tiers for sensitive captures.

Takeaways: a stated size cap is normal and should be explicit; "who talked to
whom, and what service was it" is the headline framing; privacy of the capture is
a real user concern worth stating (ours is answered structurally — the capture is
processed locally in the sandbox).

### 3. PCAP Analyzer for Splunk

Dashboard-shaped take on the same data, and the clearest precedent for the
"busiest ports" deliverable, which Wireshark has no dedicated table for: it ranks
top talker IPs, top MAC addresses, top protocols, **top ports**, VLANs, and
conversations, plus TCP error and packet-loss views.

Takeaways: **top ports is a first-class ranked table** (protocol + port +
volume), and MAC-level talkers are expected alongside IP-level ones when the
capture is Ethernet.

(Also skimmed for framing, not counted among the three: a Streamlit/scikit-learn
pcap analyser that adds a protocol pie chart, byte totals per IP, port→service
naming, a port-scan flag, and an unsupervised anomaly score — it documents no
tunable thresholds and states it loads the whole file into memory, so it suits
small-to-medium captures only.)

## Table stakes → decision

Every table stake lands in the descriptor or in the out-of-model list below.
Nothing is silently dropped.

| Table stake | Seen in | Decision |
| --- | --- | --- |
| Capture-level properties (format, encapsulation, snaplen, packets, bytes, first/last time, duration) | capinfos | **In** — `overview` section |
| Average packet size, packets/s, bits/s | capinfos | **In** — `overview` |
| Snaplen truncation visible | capinfos | **In** — `snaplen` + `truncated_packets` in `overview` |
| Protocol breakdown with packets **and** bytes | tshark `io,phs`, all three | **In** — `protocols`, per layer, with packet and byte percentages |
| Layered protocol hierarchy (tree) | tshark `io,phs` | **In** — `hierarchy`, as colon-joined layer paths (`eth:ipv4:tcp:https`); a JSON list of paths is the flat equivalent of the GUI tree |
| Application-layer naming, not just TCP/UDP | `io,phs`, Splunk | **In** — well-known-port service naming feeds both `hierarchy` and `ports` |
| Top talkers by bytes and packets | all three | **In** — `talkers` |
| Directional sent/received per endpoint | tshark `endpoints` | **In** — `packets_sent`/`bytes_sent`/`packets_received`/`bytes_received` |
| MAC/Ethernet-level talkers | tshark `endpoints,eth`, Splunk | **In** — `mac_talkers`, emitted when the link layer is Ethernet |
| Conversations with per-direction counts | tshark `conv` | **In** — `conversations` with A→B and B→A splits |
| Conversation relative start + duration | tshark `conv` | **In** — `start_seconds`, `duration_seconds` |
| Busiest ports ranked | Splunk | **In** — `ports` (protocol, port, service, packets, bytes, endpoint count) |
| Choice of sort column | Wireshark GUI (click-to-sort), tshark's fixed sorts | **In** — `sort_by` = `bytes` \| `packets` |
| Top-N rather than everything | Splunk dashboards, all summary UIs | **In** — `top`, default 10, with `*_total` beside every list so a cap is never mistaken for the whole picture |
| Ability to look at one table at a time | separate tshark `-z` invocations; separate dashboard panels | **In** — `section` enum (`all`, `overview`, `protocols`, `talkers`, `conversations`, `ports`) |
| Port→service names | Splunk, Streamlit tool | **In** — `resolve_ports` boolean, default on |
| Stated capture size cap | A-Packets (~25 MB free) | **In** — 32 MiB, stated in the descriptor and in the error text |
| pcap **and** pcapng input | all three | **In** — both containers, both byte orders, µs and ns timestamps |

### Out of model — listed, not built

* **Display-filter argument** (`tshark -z conv,ip,<filter>`): needs Wireshark's
  full dissector/filter language. `pcap-grep` already covers payload-level
  selection, and `section` + `top` cover the summary need.
* **TLS/HTTPS decryption**: requires key material the tool is never given;
  encrypted flows are counted and named by port, never decrypted.
* **Charts / pie charts / Sankey / I/O graphs / host graphs**: this block's
  surfaces are chat + CLI returning JSON; there is no page for a binary
  file → JSON tool (0 of the repo's `Input::File` blocks have one), so
  visualisation belongs to the consumer, not here.
* **ML anomaly detection and port-scan scoring** (Streamlit tool's isolation
  forest; Splunk's error dashboards): needs a model, which is out of gizza's
  pure-Rust model; also undocumented thresholds make it unreproducible.
* **TCP expert analysis** (retransmissions, zero-window, RTT, packet loss):
  needs per-stream sequence tracking and state machines — a different tool's
  worth of work, deliberately not smuggled into a summary.
* **Reverse DNS / geolocation / OUI vendor lookup for talkers**: needs network
  access or a bundled dataset.
* **Wireless artefacts** (SSIDs, WPA handshakes): 802.11 link-layer dissection;
  the link layers decoded here are Ethernet, raw IP, Linux cooked, and
  null/loopback.
* **File hashes of the capture itself** (`capinfos -H`): available from the
  existing `file-hash` block; kept out to keep this core dependency-free.
* **Credentials, DNS question lists, HTTP request lines, carved files**: already
  shipped by `pcap-network-forensics` and `pcap-file-extractor` — deliberately
  not duplicated here.

## UX / defaults borrowed as ideas (not copy)

* Default to a **complete** summary (`section=all`) so one call answers the row's
  question, the way the desktop reference shows the capture properties dialog
  without configuration.
* Rank by **bytes** by default: volume is what "top talker" and "busiest port"
  mean in every one of the three tools; packets is the one-flag alternative.
* Keep **top=10**: dashboard panels show a short ranked list, not the full table.
  Every list is accompanied by its untruncated total.
* Report **both** packets and bytes on every ranked row, plus percentages on the
  protocol tables — percentages are what make a breakdown readable.
* Name services from well-known ports by default, since a bare `443` is less
  useful than a labelled one; switchable off for raw numbers.

## Stated limits (surfaced in the descriptor and in the FAQ-equivalent doc comments)

* Captures are capped at 32 MiB; larger files are rejected with an actionable
  message.
* Encrypted payloads are summarised by port, never decrypted.
* Only the first IP fragment carries transport headers, so later fragments count
  toward IP totals but not toward port/conversation tables.
* Link layers decoded: Ethernet (incl. stacked VLAN tags), raw IP, Linux cooked,
  null/loopback. Other link types still count in the overview totals.
* Timestamps come from the capture; a capture written without them yields a zero
  duration and zero rates rather than a fabricated one.
* Surfaces: chat + CLI. No page — binary file → JSON has no page shape in this
  repo (verified: 0 of the `Input::File` blocks have `page/`), so no Playwright
  spec applies.

# pcap-network-forensics — competitor analysis (2026-07-10)

Scope of our tool: an **offline, single-file** pcap/pcapng parser + aggregator that runs
pure-Rust in the browser/chat Service Worker and the CLI. No live capture, no ML, no server.
It produces a forensic summary: host inventory, conversations, DNS queries, HTTP requests, and
basic cleartext credential findings. (Paraphrased research only — no competitor copy/branding.)

## Competitors surveyed

| Tool | Kind | Relevant surface |
| --- | --- | --- |
| NetworkMiner (Netresec) | Desktop NFAT | Hosts / Sessions / DNS / Credentials / Files tabs from a pcap |
| A-Packets (apackets.com) | Online analyzer | Upload pcap/pcapng → HTTP, DNS, Credentials, Files, host relationships |
| BruteShark | Desktop/CLI forensic | Credential + hash extraction, TCP session reconstruction, network map |
| tshark / Wireshark | CLI + GUI | Endpoints/Conversations statistics, DNS/HTTP dissectors, follow-stream |
| chaosreader | Perl CLI | Any-snarf of TCP/UDP sessions (telnet, FTP, HTTP) from a capture |

## Table-stakes capabilities (tagged for gizza's model)

### In-model — built into this tool
- **Host / endpoint inventory** with address, packet count, byte count (NetworkMiner Hosts tab,
  tshark `-z endpoints`). → `hosts[]` with `address`, `packets`, `bytes`.
- **Conversations / sessions** aggregated by 5-tuple with packets + bytes (tshark `-z conv`,
  NetworkMiner Sessions). → `conversations[]` (bidirectional endpoint pair + ports + counts).
- **DNS query decoding** — question name + record type; response A/AAAA addresses when present
  (NetworkMiner DNS tab, A-Packets DNS view). → `dns_queries[]`.
- **HTTP request extraction** — method, host, path, user-agent (A-Packets HTTP view, tshark
  `http.request`). → `http_requests[]`.
- **Cleartext credential findings** — HTTP Basic `Authorization` (base64-decoded), HTTP POST
  form logins (username/password fields), FTP `USER`/`PASS`, POP3 `USER`/`PASS`
  (NetworkMiner/BruteShark/A-Packets Credentials tab, subset). → `credentials[]`.
- **pcap AND pcapng containers**, Ethernet / raw-IP / Linux-cooked / loopback link layers,
  IPv4+IPv6 (all competitors). → reused parsing approach.
- **Output caps / focus** — competitors paginate tabs; we expose `limit` (per-section entry cap)
  and `section` (all|hosts|conversations|dns|http|credentials) so an LLM/CLI can focus one view.

### Out-of-model — listed, not built (needs a backend, ML, or heavy TCP reassembly)
- **File / artifact carving** from HTTP/FTP/SMB streams (needs full multi-segment TCP reassembly
  and disk output) — NetworkMiner/A-Packets Files tab.
- **NTLM / Kerberos / MSSQL hash extraction + Hashcat formatting** (BruteShark) — beyond basic
  cleartext creds; deferred.
- **TLS/SSL metadata (SNI, JA3/JA3S fingerprints)** — deferred (a bounded SNI grab could be a
  future add; not in this first version).
- **SMB / SMTP / IMAP / Telnet full dissection** — broader protocol suite; deferred.
- **GeoIP / ASN enrichment** — needs an external database.
- **Visual network map / graph** and interactive tab UI — gizza tool has no page (file→JSON
  chat+CLI shape, like parse-pcap); the `section` param is the headless analogue of tabs.
- **Live / interface capture** — explicitly out of scope (offline single file only).

## Defaults / UX notes adopted
- Competitors auto-analyze the whole file with no required options; we default to processing the
  whole capture and returning every section, capping each list at `limit=100` entries.
- Credential findings echo protocol + endpoints + username/password exactly as observed on the
  wire (these are already-cleartext by definition) — matching how NetworkMiner/A-Packets present
  the Credentials tab. We state the "unencrypted protocols only" limitation in the tool text.
- Sibling tool `parse-pcap` already gives the per-packet decode (a different lens); this tool is
  the aggregated forensic report, so it does not duplicate the packet dump.

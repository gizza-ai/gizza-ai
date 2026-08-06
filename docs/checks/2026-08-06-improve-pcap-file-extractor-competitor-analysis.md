# pcap-file-extractor — competitor analysis (2026-08-06)

Scan run BEFORE implementation, per `/create-next-tool` step 4. Everything below is a
**paraphrase** of publicly documented behaviour; no competitor copy, branding, or trademark text
is reproduced or reused.

## Scope of the backlog row

> Reassembles and carves files transferred over HTTP, FTP, and SMB inside a PCAP capture for
> download.

## Duplicate / viability check (done first)

| Existing block | Overlap | Verdict |
| --- | --- | --- |
| `pcap-network-forensics` | Parses the same containers, reports hosts/conversations/DNS/HTTP request lines/cleartext creds. Its own skill description states: *"no file carving"*. | Not a dup — disjoint output. |
| `parse-pcap` | Per-packet decode summary only; no stream reassembly. | Not a dup. |
| `pcap-grep` | Regex over individual packet payloads; no reassembly, no objects. | Not a dup. |
| `carve-files` | Magic-byte carving of a **raw blob**. It cannot see a file split across TCP segments, de-chunk HTTP, or gunzip a body. | Not a dup — complementary (raw-blob carving vs. protocol-aware reassembly). |

Model fit: pure Rust (pcap/pcapng + TCP + HTTP/FTP/SMB2 parsing, `flate2`, `sha2`, `md-5`,
`base64` — all already proven wasm-safe in this repo), single file in, JSON out with recovered
bytes inline as base64. This is exactly the shape `carve-files` already ships (N recovered files,
each with offset/size/type + inline base64 within a budget), so the "for download" requirement maps
onto an established, supported output. File-input → JSON is the **no-page** pattern here
(`parse-pcap`, `pcap-grep`, `pcap-network-forensics`, `carve-files` all ship chat + CLI only), so
this tool has no standalone page and no Playwright spec — stated explicitly rather than claimed.

## Competitors reviewed

1. **Wireshark / tshark — "Export Objects"** (`File → Export Objects`, `tshark --export-objects
   <proto>,<dir>`). Reachable, documented.
2. **NetworkMiner** (Netresec) — network forensic analysis tool, "Files" tab.
3. **A-Packets** — browser-based pcap analyser with artifact extraction.

### 1. Wireshark / tshark — Export Objects

- Protocol pickers offered in the GUI menu: HTTP, SMB (and SMB2), IMF (email), FTP-DATA (added in
  4.0), plus TFTP/DICOM in the tshark protocol list.
- Object list columns: **packet number, hostname, content type, size, filename**.
- A free-text filter box narrows the object list.
- Saving writes each object to a directory; duplicate names get a numeric suffix.
- SMB export shows a **completion percentage** per object; anything below 100 % means the transfer
  was not fully captured and the bytes are unusable.
- Bodies are de-chunked and decompressed before export (the dissector reconstructs the entity body).
- Workflow is: apply a display filter to find the transfer, then export.

### 2. NetworkMiner

- Extracts files from a notably wider protocol set: FTP, HTTP, HTTP/2, IEC-104, IMAP, LPR, POP3,
  SMB, SMB2, SMTP, TFTP.
- Beyond files it surfaces images (thumbnail tab), credentials, X.509 certificates, DNS, and hosts.
- Per-artifact metadata is the selling point: filename, size, source/destination host, protocol,
  timestamp, and **file hashes** for malware triage.
- Exports the artifact index to CSV/Excel/XML/JSON-LD.
- Offline, single-capture analysis; the free edition is a desktop GUI.

### 3. A-Packets

- Online analyser: upload a capture, get HTTP session reconstruction and pull out images/documents
  /payload artifacts from HTTP flows without manual reassembly.
- Free tier caps uploads at ~25 MB and publishes results on a **public** page; privacy requires a
  paid/on-prem tier.
- Also does host/service mapping, DNS, credential detection, wireless artifacts.
- Extraction is centred on HTTP; FTP/SMB objects are not advertised.

## Table stakes → decision

| # | Table stake (who ships it) | Decision |
| --- | --- | --- |
| 1 | Extract HTTP response bodies (all three) | **In model** — implemented. |
| 2 | Extract HTTP request bodies / uploads (Wireshark, NetworkMiner) | **In model** — implemented, tagged `direction: "upload"`. |
| 3 | De-chunk `Transfer-Encoding: chunked` (Wireshark) | **In model** — implemented; recorded in `decodings`. |
| 4 | Decompress `Content-Encoding: gzip`/`deflate` (Wireshark) | **In model** — implemented via `flate2`; recorded in `decodings`. `br`/`zstd` are reported un-decoded with a note. |
| 5 | FTP data-channel extraction incl. PASV/EPSV/PORT/EPRT (Wireshark 4.0 FTP-DATA, NetworkMiner) | **In model** — implemented: the control channel is parsed for `RETR`/`STOR`/`LIST` + the negotiated endpoint, and the matching data connection is emitted with the real filename. |
| 6 | SMB file extraction (Wireshark SMB2, NetworkMiner) | **In model** — SMB2/3 implemented (CREATE → name + `FileId`, READ/WRITE → sparse offset assembly over `445`/`139` NBSS framing). SMB1/CIFS is **detected and reported in `notes`, not carved** — stated as a limit, not silently dropped. |
| 7 | Per-object columns: packet number, host, content type, size, filename (Wireshark) | **In model** — every one of those fields is on each result row. |
| 8 | Text filter over the object list (Wireshark) | **In model** — `filter` param, case-insensitive substring over filename/content-type/host/URI. |
| 9 | Transfer-completeness percentage (Wireshark SMB) | **In model** — `complete` + `completeness_percent` on every object, from TCP-reassembly gaps and SMB sparse coverage; `include_incomplete` toggles them. |
| 10 | File hashes for triage (NetworkMiner) | **In model** — MD5 + SHA-256 per object. |
| 11 | Content-type vs. real magic bytes (implicit in every carving tool) | **In model, differentiator** — `detected_type` sniffs the recovered bytes and `type_mismatch` flags a body whose declared MIME disagrees (e.g. an `.exe` served as `text/plain`). |
| 12 | Get the bytes out (all three) | **In model** — recovered bytes inline as base64 within a budget, the shipped `carve-files` shape. |
| 13 | Protocol picker per export (Wireshark's per-protocol menus) | **In model** — `protocols` param (`all` or a comma list of `http,ftp,smb`). |
| 14 | Skip noise / cap results (all three) | **In model** — `min_size`, `limit`, `include_content`, `max_content_bytes`. |
| 15 | Email objects: SMTP/IMAP/POP3/IMF (Wireshark IMF, NetworkMiner) | **Out of scope for this row** — the row names HTTP/FTP/SMB. Listed, not built. Would be a separate block. |
| 16 | TFTP / DICOM / IEC-104 / LPR objects (Wireshark, NetworkMiner) | **Out of scope** — same reason. Listed, not built. |
| 17 | Live capture from an interface | **Out of model** — no raw sockets in a browser/wasm sandbox. Listed, not built. |
| 18 | TLS decryption with a key log to reach HTTPS objects | **Considered, not built** — technically pure-Rust-able but a large second engine (TLS 1.2/1.3 record + key-schedule) and needs a second file input (the `SSLKEYLOGFILE`), which the single-file input shape does not carry. Stated as a limit on the tool. |
| 19 | Writing each object to its own file on disk / bulk "save all" | **Out of model** — one JSON artifact per call is the platform's output contract; base64 rows are the equivalent and are what a caller pipes to disk. |
| 20 | Cloud upload + a public results page (A-Packets) | **Out of model, deliberately** — this runs locally; nothing is uploaded. |
| 21 | GUI thumbnail/preview gallery (NetworkMiner, A-Packets) | **Out of model here** — this is a no-page file-input block (chat + CLI). |

## UX / defaults borrowed as ideas (not copy)

- Wireshark's object-list columns drove the result-row field set (packet, host, content type, size,
  filename) so the output reads like the list an analyst already knows.
- Wireshark's SMB completeness percentage generalised to **every** protocol here, since TCP gaps
  affect HTTP and FTP objects identically.
- NetworkMiner's hash column drove MD5 + SHA-256 on every row.
- A-Packets' 25 MB free cap informed the documented input ceiling (32 MiB) and the reassembly /
  inline-content budgets, which are stated in the parameter descriptions and in the error text
  rather than discovered by failure.

## Stated limits (surfaced in the descriptor and in error messages)

- Encrypted transports (HTTPS/TLS, SMB3 encryption, FTPS/SFTP) yield nothing — no key material.
- SMB1/CIFS is detected and reported, not carved.
- IPv4 fragments after the first are skipped; UDP-borne protocols (TFTP, QUIC) are not handled.
- Capture ceiling 32 MiB; reassembled-stream budget 12 MiB; default inline-content budget 4 MiB
  (max 16 MiB) — all reported in the response so a truncated run is never silent.

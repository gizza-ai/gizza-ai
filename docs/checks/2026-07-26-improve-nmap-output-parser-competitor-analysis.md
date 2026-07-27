# nmap-output-parser — competitor analysis (2026-07-26)

Snapshot taken while building the tool. Profiles are **paraphrased** — no competitor copy,
branding, or assets were reused. gizza's `nmap-output-parser` is browser-local, pure-Rust/WASM,
no account, no server, text-in → table-out.

## Competitors surveyed

1. **nmap-formatter** (vdjagilev/nmap-formatter) — Go CLI. Input: nmap **XML only**. Output: HTML,
   CSV, JSON, Markdown, Graphviz DOT, SQLite, Excel, D2. `--skip-down-hosts` default on. A
   converter, not an interactive viewer.
2. **nmap-parse-output** (ernw) — XSLT-driven CLI over nmap/masscan XML. Output: HTML, JSON, and
   plain-text extractions (IP/port/service lists). Strong filtering: include/exclude hosts/ports,
   group by port/service/product, diff vs a prior scan, enriched TLS/HTTP fields.
3. **nmaptocsv** (maaaaz) — Python. The most flexible **input**: normal text (`-oN`), greppable
   (`-oG`), and XML (`-oX`). Output: CSV, custom delimiter, user-selectable column superset
   (`fqdn, rdns, ip, mac, port, protocol, os, script, service, version`).
4. **NmapView** (nmapview.github.io) — browser-local XSLT HTML report. Input: XML. Interactive
   sort/filter/global-search table; CSV/JSON/Excel export; fully client-side (nothing uploaded).
   Closest analog to a browser-local table tool.
5. **WebMap** (noraj) — Docker web dashboard over XML. Charts, PDF reports, CVE lookup, host
   notes, scan scheduling. Backend-required; explicitly not browser-local.

## Gap list → decisions

### Built (in-model)

- **Multi-format text ingestion:** nmap **XML (`-oX`)** and **greppable (`-oG`)**, with
  **auto-detect** so users just paste. (Most competitors take XML only; multi-format is the
  nmaptocsv edge.)
- **Core column union:** host IP, hostname/PTR, port, protocol, state, service, and a joined
  product+version+extrainfo "Version" column — the fields every competitor shows.
- **Three structured outputs:** Markdown table, CSV (RFC-4180 quoted), JSON array — the requested
  trio; no single competitor ships all three from a browser-local tool.
- **Open-ports-only toggle** (default on, matching nmap-formatter's skip-down default) and a
  **sort-by host / port / service** control (host sorts IPv4 numerically so `.9` precedes `.10`).
- **MAC-address suppression:** the `<address addrtype="mac">` row never becomes the host IP.
- **Client-side privacy** stated explicitly on the page (nothing uploaded — WASM in-browser).

### Considered, not built

- **Normal-text (`-oN`) ingestion** (nmaptocsv) — deferred: the human-readable format is far less
  regular than XML/greppable and its port table wraps unpredictably; XML/greppable cover the two
  machine-readable outputs users script against. Noted as a future add.
- **Extended columns** — OS/CPE guess, MAC vendor, hop number, reason, NSE script output, TLS cert
  CN/SAN, HTTP title. Present in XML and in-model, but they explode the table width; kept the core
  7-column view. Candidate for a future "columns" selector.
- **Group-by / global search / diff-vs-previous** (nmap-parse-output, NmapView) — interactive
  table features that belong to a richer results grid; the current tool renders a single sorted
  table. In-model but out of this scope.
- **Copy-as-command presets** (curl/nikto templating, WebMap) — pure string templating and
  in-model, but a distinct feature surface; deferred.

### Out-of-model (needs a backend / live data)

- CVE/exploit lookup by CPE, host labels/notes persistence, scan scheduling, running scans, REST
  API, rarity scoring, Excel/SQLite/PDF binary exports, Graphviz/D2 diagrams.

## Net position

The white-space a browser-local pure-Rust tool owns is the intersection no single competitor
covers: **multi-format text ingestion + Markdown/CSV/JSON out + sort/filter, all client-side with
zero upload.** This tool ships that intersection; richer interactive-grid and backend features are
explicitly deferred above.

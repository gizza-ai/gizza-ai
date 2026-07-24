# log-format-converter — dup-check / skiplist analysis (2026-07-24)

**Backlog row:** `log-format-converter` — "Converts access logs between combined, common, and JSON formats and exports to CSV for spreadsheets." (type: pure)

## Decision: SKIPLIST — semantic duplicate of `blocks/log-parser`

### Why

`blocks/log-parser` already covers the entire stated purpose of this backlog row.
Confirmed against its source:

- **Input formats** (`Format` enum, `blocks/log-parser/src/lib.rs`): `auto`,
  `json`/`ndjson`, `logfmt`, `syslog`, `common` (Apache/nginx Common Log Format),
  `combined` (Common + referer + user-agent). Its own param `.describe()` text
  advertises forcing `common`/`combined`/`json`.
- **Output shapes** (`Output` enum): `table` (Markdown), `json` (array, one object
  per line), `csv` (header + rows). The param `.describe()` reads
  *"'json' is an array of one object per line; 'csv' is header + rows."*
- Module doc: *"parses each line into a normalized set of fields, then renders a
  filterable structured view as a Markdown table, a JSON array, or CSV."*

So the log-format-converter's headline capabilities are already shipped:

| Backlog promise | Covered by log-parser |
|---|---|
| read `combined` access logs | yes (`Format::Combined`) |
| read `common` access logs | yes (`Format::Common`) |
| read `json` logs | yes (`Format::Json`) |
| convert → JSON | yes (`Output::Json`) |
| **export to CSV for spreadsheets** | yes (`Output::Csv`) |
| auto-detect the format | yes (`Format::Auto`, majority vote) |

### The only genuine delta (and why it doesn't justify a second tool)

The lone capability log-parser does **not** offer is *re-emitting* `combined`/`common`
access-log **lines** as output (i.e. a format downgrade: combined→common line, or
json→combined line). That output rendering is in-model (pure string formatting), but
it is a marginal add-on, not the row's headline — the description leads with
"exports to CSV for spreadsheets," which is exactly `log-parser --output csv`. Shipping
a near-identical parser to gain one extra output renderer would bloat the catalog with
a confusable near-duplicate. If line-format output is ever wanted, it belongs as an
added `Output::Combined`/`Output::Common` variant on log-parser, not a separate tool.

### Related existing blocks (all overlap the log-normalization space)

- `blocks/log-parser` — parse+normalize+render (table/json/csv) — the direct dup.
- `blocks/log-analyzer` — auto-detects format, analyzes.
- `blocks/log-merger`, `blocks/ip-log-anonymizer` — adjacent log utilities.

### Competitor landscape (paraphrased, not built)

Typical "log format converter" web tools do exactly what log-parser does: paste
Apache/nginx access logs → pick output (JSON / CSV / table), with format
auto-detection and a CSV download for spreadsheets. No table-stake capability exists
here that log-parser lacks except the marginal line re-emit noted above. Nothing
copied; no branding/copy reproduced.

**Result:** `skiplisted log-format-converter: duplicate of blocks/log-parser`.

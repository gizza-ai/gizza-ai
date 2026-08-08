# json-log-formatter — competitor analysis (2026-08-08)

Scan run **before** implementation, per the create-next-tool procedure. Everything below is
**paraphrased** from public docs/product pages — no competitor copy, branding, or trademarked
wording is reused anywhere in this tool's page, descriptor, or output.

## Why this tool is not a duplicate (pre-build check)

`ls blocks/ | grep -iE 'json|log'` + reading the neighbours' cores:

| Existing block | What it already does | What it does NOT do |
| --- | --- | --- |
| `log-parser` | Auto-detects JSON/NDJSON, logfmt, syslog, Apache/nginx; minimum-severity filter; whole-line substring/regex filter; Markdown table / JSON / CSV; row limit. | Nested objects are stringified into one cell (no dotted flattening); no per-field filter; no custom level/time/message key names; no column selection; no aligned plain-text log view. |
| `ndjson-filter` | Predicate language (`age > 28 and city == NYC`), dotted-path field select/rename, NDJSON/array/CSV, invert, limit. | No severity model at all; no aligned/table *readable* rendering; no flattening; no log-specific field detection. |
| `json-from-logs` | Recovers embedded `{…}`/`[…]` blocks out of mixed console text. | Not a per-line NDJSON renderer. |
| `log-analyzer` | Aggregate stats (level counts, top errors, timeline). | Not a per-record view. |
| `jsonl-deduplicator`, `json-beautify`, `log-to-table` | Dedupe / single-document pretty print / regex column extraction. | — |

The differentiated core here is: **NDJSON in → aligned human-readable log lines out**, with
dot-flattening of nested objects, explicit `level`/`time`/`message` key selection, and a
per-field (path) contains/exact filter. That combination exists in none of the blocks above.

## Competitors reviewed

1. **bunyan CLI** (`bunyan` npm, node-bunyan man page) — the reference "filter + pretty-print JSON
   log file" CLI.
2. **pino-pretty** (pinojs/pino-pretty) — the most-used NDJSON prettifier for Node.
3. **jq** as used for log triage (`select(.level=="ERROR") | {…}`) — the generic answer people reach
   for; several tutorials/cheat-sheets cover exactly this workflow.
4. **Browser JSONL/NDJSON viewers** — devtooleasy JSON Lines Viewer, OpenFormatter NDJSON Viewer.
5. **Hosted JSON log viewers** — Log Voyager, jsonformatter.online's JSON log viewer, LogViewPlus
   (desktop).

## Table stakes (paraphrased)

- One JSON value per line; each line parses independently, a bad line must not kill the run.
  Blank lines are skipped silently; CRLF and LF both accepted. (viewers, bunyan)
- A **minimum-severity filter** by level name, and support for numeric levels (bunyan uses
  10/20/30/40/50/60 for trace…fatal; accepting both names and numbers is table stakes). (bunyan, pino)
- **Special/known fields** get dedicated treatment rather than being just more columns —
  time, level, message (plus name/hostname/pid in bunyan). (bunyan, pino-pretty)
- **Configurable key names**: pino-pretty exposes `levelKey`/`messageKey`/`timestampKey` because
  every ecosystem picks different names (`msg` vs `message`, `time` vs `ts` vs `@timestamp`).
- **Field include/exclude**: pino-pretty's `include`/`ignore` (dot notation, e.g. `req.headers`);
  jq's `{time: .timestamp, msg: .message}` projection. Column selection is expected.
- **Multiple output shapes**: bunyan ships long / short / json / json-N / inspect; the browser
  viewers ship a card view and a table view; jq users pipe to CSV. A single "pretty" shape is not
  enough.
- **A substring search across everything**, including inside nested values (both browser viewers
  serialize the record and match against it).
- **Client-side only / nothing uploaded** is an explicit selling point of every browser tool
  (production logs and PII are the stated reason).
- **Per-line validity feedback** — viewers show valid/invalid counts and keep invalid lines
  visible rather than silently dropping them.

## Defaults observed

| Competitor | Notable defaults |
| --- | --- |
| pino-pretty | `levelKey=level`, `messageKey=msg`, `timestampKey=time`, `ignore=pid,hostname`, `singleLine=false`, `hideObject=false`, `levelFirst=false`, `translateTime=false`, no minimum level. Line shape: `[17:35:28.992] INFO (42): hello world`, then the remaining object below/after. |
| bunyan | `-o long` default; non-JSON lines are **passed through** unless `--strict`; `-l` accepts a name or a number; timestamps UTC unless `-L/--time local`. |
| jq | Pretty, indented, colorized output by default; `-c` for one compact object per line. |
| Browser viewers | Table view auto-derives columns from the union of keys across records; nested objects collapse to a badge; substring filter is case-insensitive; blank lines skipped. |

## Examples competitors lead with (paraphrased shapes only)

- Tail-a-service shape: a few INFO lines with a couple of `key=value` extras, then one ERROR with an
  error object — used to show the level colouring/alignment payoff.
- "Show me only the errors": minimum level = error, output stays readable.
- "Project three fields": timestamp + level + message only, with everything else dropped.
- Nested-context shape (`req.method`, `req.url`, `user.id`) — used to show dot-path handling.

## UX patterns worth adopting

- Level rendered as a fixed-width upper-case token so eyes can scan a column (all pretty-printers).
- Time in brackets, first, then level, then message — a layout users already read fluently.
- Show the record's leftover fields as compact `key=value` pairs rather than a JSON blob.
- Table view derives its columns from the first-seen union of keys, so heterogeneous records still
  line up (both browser viewers).
- State the practical size limit up front (one viewer says ~25 MB works well; another says rendering
  tops out around 5–10k rows) instead of letting the user discover it by hanging the tab.
- Keep invalid lines accounted for (count/notice) instead of silently vanishing them.
- Sample/"load example" button — our `[[example]]` chips are the equivalent.

## In-model (built here)

| Capability | How it lands |
| --- | --- |
| Aligned readable log view | `output=pretty` (default): `[time] LEVEL message key=value …`, time/level/message columns padded to line up. |
| Table / JSON / CSV shapes | `output=table` (padded Markdown + caption), `json` (pretty array), `csv` (RFC 4180). Covers bunyan's `-o` family and the viewers' table view. |
| Minimum severity | `level` enum `all|trace|debug|info|warn|error|fatal`; accepts level **words** and **numbers** (10–60 bunyan-style, and 0–7 syslog-style) in the data. |
| Configurable key names | `level_field`, `time_field`, `message_field` — blank = auto-detect across the common aliases. Mirrors pino-pretty's `levelKey`/`timestampKey`/`messageKey`. |
| Per-field filter | `field` (dot path) + `filter` value + `match` = `contains` (case-insensitive) or `exact`. Blank `field` searches the whole record, matching the viewers' global search. |
| Column selection | `fields` — comma-separated dot paths, ordered, missing paths render empty. Mirrors pino-pretty `include` / a jq projection. |
| Flatten nested objects | `flatten` (default true) → `req.method`, `items.0.id`. Off = nested values render as compact JSON. This is the gap none of our existing blocks fill. |
| Row cap | `limit` 1–5000, default 200 — the explicit answer to "rendering tops out around 5–10k rows". |
| Bad-line policy | `on_invalid` = `skip` (default) / `keep` (bunyan's pass-through) / `error` (names the line number). |
| Epoch timestamps | Numeric `time` values are rendered as ISO 8601 UTC (pino writes epoch ms by default). |
| Private by construction | Runs as wasm in the page/CLI; no network at all. |

## Considered, not built (out-of-model or rejected)

- **Colourised output** — the shared page renders the result as plain text; ANSI codes would be
  noise in the CLI-piped and copy-paste cases. Alignment carries the scannability instead.
- **A full expression language** (`bunyan -c 'this.pid == 123'`, arbitrary jq programs) — that is
  `ndjson-filter`'s job in this repo; duplicating it would bloat this schema. Out of scope by
  design, and named as such on the page.
- **Interactive sort-by-column / expand-a-row cards** — needs stateful client UI beyond the shared
  declarative page runtime; rejected rather than adding a per-tool JS escape hatch.
- **File upload of multi-MB log files / streaming** — the shared text page takes pasted text; a file
  picker for text tools isn't in the platform's model. The `limit` cap plus the stated limits on the
  page cover the practical case.
- **Live tailing / following a stream, DTrace-style process attach** (bunyan `-p`) — needs a server
  or a local process; out of model for a browser-local wasm tool.
- **Colour themes / custom level vocabularies beyond the six standard names** — deferred; custom
  level names still render verbatim, they just sort as `info` for the minimum-severity filter.

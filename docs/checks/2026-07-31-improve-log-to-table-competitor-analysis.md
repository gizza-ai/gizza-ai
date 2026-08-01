# log-to-table — competitor analysis (2026-07-31)

Function: parse semi-structured log lines into a CSV/table using a regex template
with named capture groups, plus presets for common log formats. All findings
paraphrased — no competitor copy, branding, or trademarks reproduced.

## Top 3 competitor tools (paraphrased)

1. **Grok Constructor** (grokconstructor.appspot.com) — pastes several logfile
   lines and matches them against a grok/regex expression, then prints the named
   groups extracted from each line side by side. Emphasis on trying an expression
   against many lines at once and seeing which lines match. Free, browser-based.

2. **Grok Pattern Builder & Debugger** (inventivehq.com) — build/test/debug a grok
   pattern with live match highlighting, "fix suggestions," a library of ready
   patterns for common logs (nginx, apache, syslog…), and export to plain regex /
   Logstash config / Elasticsearch ingest pipeline.

3. **Grok Pattern Tester** (bitsentry.ai) — `%{PATTERN:field}` grok syntax that
   compiles down to named capture groups; tests multiple log lines and lists the
   captured fields per line. (Kibana's built-in Grok Debugger is the same class of
   tool, hosted inside Elastic.)

## Table-stakes / defaults / examples / UX controls

| Capability | Competitors | Our decision |
|---|---|---|
| Multi-line log input | all | **in-model** — `logs` multiline textarea |
| Regex with **named groups → columns** | all (grok compiles to this) | **in-model** — `(?P<name>...)` template, first match per line |
| Presets for common formats (apache common/combined, syslog, log4j) | 2 & 3 ship pattern libraries | **in-model** — `preset` enum supplies a ready regex; `custom` uses your own |
| Table / structured output | all (per-line field table) | **in-model** — `output` = table / csv / tsv / json |
| Header row toggle | CSV/table tools | **in-model** — `header` boolean |
| Handling of non-matching lines | Grok Constructor highlights unmatched | **in-model** — `on_nomatch` = skip / keep / error |
| Row cap for huge pastes | implicit | **in-model** — `limit` (1–5000, default 500) |
| Worked examples / preset chips | "Try it" links | **in-model** — 4 `[[example]]` chips |

## In-model vs out-of-model decisions

**In-model (built):** `logs`, `preset` (custom/common/combined/syslog/log4j),
`pattern` (regex template), `output` (table/csv/tsv/json), `header`, `on_nomatch`
(skip/keep/error), `limit`. Named-group→column extraction, CSV RFC-4180 quoting,
aligned Markdown table, JSON array of objects. Runs fully in-browser, no upload.

**Out-of-model / considered, not built:**
- **Full grok `%{PATTERN:field}` syntax** — would require bundling the whole grok
  pattern dictionary (100+ named sub-patterns) and a compiler. The `preset` list +
  raw named-group regex cover the common cases without that weight; listed here as
  a deliberate omission, not silently dropped.
- **Export to Logstash / Elasticsearch ingest pipeline config** — belongs to a
  server ingestion stack, out of a browser-local converter's model.
- **Live per-character match highlighting** — a bespoke editor UI beyond the
  generic declarative page; the per-line table already shows what matched.
- **Backreferences / lookaround in the regex** — the Rust `regex` engine is
  linear-time and does not support them; stated as a limit on the page.

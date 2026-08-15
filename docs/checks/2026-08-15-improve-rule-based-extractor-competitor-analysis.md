# rule-based-extractor — competitor analysis (2026-08-15)

Scan run **before** implementing, per the create-next-tool recipe. Everything below is
paraphrased from public documentation; no competitor copy, branding, or trademarks are reused.

**Tool being built:** applies user-defined regex/pattern rules to text and returns the extracted
fields as structured data (JSON / CSV / readable report).

## Competitors skimmed

| # | Tool | What it is | Notes |
|---|------|-----------|-------|
| 1 | Elastic / OpenSearch **grok processor + Grok Debugger** | The canonical rule-based extractor: a list of grok expressions with `%{PATTERN:field}` placeholders turns an unstructured line into named fields. The debugger is the interactive page version. | Richest feature set; documented option table below. |
| 2 | **Online grok pattern builders** (javainuse, grokdebugger.com, Inventive HQ, hexmos free-devtools grok reference) | Paste a sample line + a grok expression, see the resulting JSON object of named fields. Ship a built-in pattern library, autocomplete, live highlighting, and per-vendor pattern packs (syslog, Java, HAProxy, Cisco, Postgres…). | These define the *page UX* table stakes. |
| 3 | **rexearch** (open-source rule-based extractor) | A JSON list of rules; each rule = `regex` + optional `id`, `tags`, `target_regex_group`, `repr`, `validation`. Emits `{raw, start, end, repr, rule_id, tags}` per hit. | Confirms the multi-rule (not single-pattern) shape, per-rule capture-group selection and per-rule naming. |
| 4 | **Veryfi custom regex field extraction** (checked as a 4th, since #3 is a library not a hosted tool) | Document-AI product where a user adds regex rules that populate custom fields on an extracted document. | Rule = condition + regex + target field. The OCR/document-AI half is out-of-model. |

Sources: Elastic grok-processor reference, OpenSearch Grok Debugger docs, github.com/replon/rexearch,
Veryfi FAQ "Custom Text Field Extraction using Regex".

## Table-stakes checklist

| # | Capability (competitor) | Fit | Where it landed |
|---|---|---|---|
| 1 | **Many named rules at once**, not one pattern (all four) | in-model | `rules`, one rule per line — this is the whole point of the tool and what separates it from `regex-extract` / `regex-capture-to-csv` |
| 2 | `%{PATTERN:field}` named-pattern placeholders (grok) | in-model | Built-in macro library (`%{IPV4:client}`, `%{TIMESTAMP_ISO:ts}`, …), 30 patterns |
| 3 | Custom **pattern definitions** (`pattern_definitions`, grok) | in-model | `@NAME = regex` lines define a reusable macro; a user definition overrides a built-in |
| 4 | Explicit **field naming** per rule (rexearch `id`, Veryfi target field) | in-model | `field = regex` shorthand |
| 5 | Capture-group selection (rexearch `target_regex_group`) | in-model | Resolution order: named group matching the field name → group 1 → whole match. Bare-pattern rules emit every named group (grok-style) |
| 6 | **Regex flags** i / m / s (every regex tool) | in-model | `ignore_case`, `multiline`, `dotall` (plus inline `(?i)` per rule) |
| 7 | First match vs **capture every match** ("return every match" advanced setting in the debuggers) | in-model | `matches = first \| all`; `all` yields a JSON array per field |
| 8 | **Per-record processing** (log pipelines run the rules per event/line) | in-model | `split = whole \| lines \| paragraphs \| pattern` + `split_pattern`; per-record output array |
| 9 | Missing-field behaviour (`ignore_missing`, on-failure) | in-model | `on_missing = skip \| null \| error` |
| 10 | **JSON output** of named fields (all four) | in-model | `output = json`, `pretty` toggle |
| 11 | Tabular export for spreadsheets (log tooling norm) | in-model | `output = csv` (RFC 4180, comma) |
| 12 | **Which rule matched / debugging trace** (`trace_match`) | in-model | `output = report` — per-rule hit counts and an explicit "never matched" list |
| 13 | Value clean-up (trim) | in-model | `trim` (default on) |
| 14 | De-duplicating repeated hits | in-model | `unique` |
| 15 | Stated size limits so a big paste can't hang the tab | in-model | 1 MB text, 200 rules, `max_records`, `max_matches` — all **hard errors**, never silent truncation |
| 16 | Drop events where nothing matched | in-model | `skip_empty_records` (default on) |
| 17 | **Preset / sample rule sets** (vendor pattern packs, sample buttons) | in-model | 4 `[[example]]` preset chips: access log, invoice fields, syslog, contact scrape |
| 18 | Multi-line paste UX | in-model | `multiline = true` on `text` + `rules` |

## Out-of-model (listed, deliberately not built)

- **Live match highlighting / autocomplete of pattern names while typing** — needs a bespoke editor
  component; the generic page generator renders declarative controls only.
- **The full 120+ Logstash pattern library and per-vendor packs** (Cisco ASA, HAProxy, MongoDB,
  Redis, Postgres, Rails…) — a curated 30-pattern set covers the dates/codes/entities this tool
  advertises; the rest is a maintenance surface, and `@NAME = regex` lets a user add any of them.
- **ML/NER entity extraction and OCR document ingestion** (Veryfi's real product) — needs a model;
  gizza is pure-Rust + ffmpeg.
- **Saved rule sets / accounts / ingest-pipeline deployment** — server-side state; this tool is
  stateless and runs locally. Deep-links (`?rules=…`) are the shareable equivalent.
- **Backreferences and lookaround** — not supported by the Rust `regex` crate at all (linear-time
  engine). Stated on the page rather than silently failing.
- **Per-rule validation callbacks** (rexearch `validation`) — arbitrary user code; out of scope for
  a sandboxed pure block.
- **Type coercion / transforms on extracted values** (grok's `:int` / `:float` suffixes) — every
  value is emitted as a string; `json-transform-rules` already does typed reshaping downstream.

## Dup check

`ls blocks/ | grep -iE 'extract|regex|rule|pattern'` — nearest neighbours inspected:

- `regex-extract` — ONE pattern, returns the list of matches. No field names.
- `regex-capture-to-csv` — ONE pattern, capture groups → CSV columns. No multi-rule set, no
  pattern library, no per-record splitting.
- `regex-bulk-match` — ONE pattern tested against many lines (pass/fail report).
- `field-extractor` — delimiter/column (cut/awk) extraction, not regex rules.
- `json-transform-rules` — rules over **JSON** input, not free text.

None of these takes a *set of named rules* over free text, so this is a distinct tool, not a dup.

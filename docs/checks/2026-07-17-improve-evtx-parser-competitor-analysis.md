# evtx-parser — competitor analysis (2026-07-17)

Tool: parse Windows Event Log `.evtx` files into readable structured JSON with filters for Event ID, provider/channel, and time bounds. Pure Rust file-input tool; chat + CLI only, no page.

## Competitors scanned

1. **evtxparser.com — EVTX Viewer & Parser / EVTX to XML/JSON/CSV** — browser-local `.evtx` viewing/conversion, raw XML inspection, filters, and CSV/JSON/XML export.
2. **Convert.Guru — EVTX Converter** — drag/drop online converter for `.evtx` to CSV, XML, or JSON.
3. **Gigasheet — Online EVTX Parser and Viewer** — parses Windows event logs and supports filtering/analysis in a spreadsheet-style UI with export-oriented workflows.
4. **MarkdownMind evtx-converter** — CLI/library-oriented EVTX parser/converter emphasizing fast/safe EVTX decoding.
5. **python-evtx / EvtxECmd-style workflows** — local forensic tools that parse EVTX records, expose XML/JSON-like event data, and commonly filter by event id/time/provider in downstream analysis.

## Table-stakes → model-fit decisions

| Capability | Competitors | Decision |
|---|---|---|
| Decode binary `.evtx` records | all | **in-model** — implemented with the pure-Rust `evtx` crate |
| JSON output | all converter tools | **in-model** — structured JSON response with record summaries and optional full parsed data |
| CSV/XML export | converter tools | out-of-scope for v1 — JSON is the gizza chat/CLI surface; downstream conversion tools can transform it |
| Filter by Event ID | viewer/forensic tools | **in-model** — `event_ids` comma list |
| Filter by provider/source and channel | viewer/forensic tools | **in-model** — `providers` substring list and `channel` exact match |
| Filter by time range | viewer/forensic tools | **in-model** — `after` / `before` ISO-8601 or date bounds |
| Cap returned records | large-log tooling | **in-model** — `max_records`, default 100, with `truncated` metadata |
| Summary/triage mode | spreadsheet/viewer tools | **in-model** — `summary=true` returns counts by event id/provider/level and time span |
| Raw XML reconstruction | evtxparser.com / forensic CLIs | out-of-scope — this tool returns parsed JSON fields; XML formatting is not required for readable analysis |
| Recursive directories / batch logs | Gigasheet/CLI tools | out-of-model for current gizza invocation — one file input per tool call |
| Saved dashboards / visual timeline UI | spreadsheet/viewers | out-of-model here — no app/dashboard surface in this repo |

## Descriptor decisions

- `event_ids`: comma-separated Windows Event IDs, e.g. `4624,4634,4688`.
- `providers`: comma-separated case-insensitive provider/source substrings.
- `channel`: exact channel filter such as `Security`, `System`, or `Application`.
- `after` / `before`: inclusive ISO-8601 or bare-date time bounds.
- `max_records`: default 100 to keep output bounded; `0` means all matched.
- `include_data`: include the full parsed record object by default; set false for compact output.
- `summary`: aggregate counts and time span instead of returning individual records.

No competitor copy, branding, or trademarks are reused beyond identifying products scanned. Every table-stake is either implemented or listed as out-of-model/out-of-scope.

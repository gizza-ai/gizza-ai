# jsonl-stats competitor analysis (2026-08-16)

Tool: `jsonl-stats` — summarizes a JSON Lines file: record count, per-key presence frequency, and value-type distribution.

## Sources scanned

- Online JSON Tools: Analyze JSON (`onlinetools.com/json/analyze-json`) — explores JSON structure by depth, extracts data type information, key counts, array values, and values at a chosen depth.
- JSON count/statistics web tools (`jsoncount.com`, OnlineToolz JSON Statistics) — emphasize counts of records/items, keys, arrays/objects, values, depth, and data types in a browser.
- Command-line/data tools (`jq`, Spark JSON reader, Miller-style data profiling patterns) — stream JSON/NDJSON records, infer schemas/types, sort/filter fields, and export tabular summaries.

## Table-stakes capabilities

| Capability / UX pattern | Seen in competitors | In current gizza model? | Decision |
| --- | --- | --- | --- |
| Record / item count | JSON counters, data-frame readers | Yes | Implemented as `records` and `lines_read`; blank lines ignored. |
| Per-key count / coverage | JSON analyzers and schema profilers | Yes | Implemented as `present` and `coverage` per key, counting records rather than repeated array occurrences. |
| Type distribution | JSON analyzers, schema inference tools | Yes | Implemented for string, number, boolean, null, object, and array counts. |
| Nested-depth exploration | Online JSON analyzer depth controls | Yes | Implemented with `depth` 1-10 and dotted paths plus `[]` array-element notation. |
| Sort controls | Table profilers | Yes | Implemented `frequency`, `name`, and `first-seen`. |
| Output as JSON/CSV/table | CLI/data tools and online tables | Yes | Implemented text, JSON, Markdown, and CSV. |
| Distinct counts and sample values | Data profiling tools | Yes | Implemented with a capped scalar distinct set and configurable sample count. |
| Numeric and string value summaries | Profilers/data-frame describe output | Yes | Implemented numeric min/max/mean and string min/max length. |
| Invalid-line handling | NDJSON guides recommend graceful handling | Yes | Implemented `report`, `skip`, and `error` modes. |
| Formal JSON Schema generation | Schema inference tools | Out-of-model for this tool | Deferred; this tool profiles observed stats but does not generate schema constraints. |
| File upload / large streaming UI | Online tools and CLI data engines | Out-of-model for this pure text page | The page accepts pasted text. CLI/chat surfaces use the same text descriptor; 50,000-line cap keeps browser and wasm memory bounded. |
| Graphical charts | Some statistics dashboards | Out-of-model | Not built; output is machine-readable tables. |

## Defaults and examples chosen

- `depth=1` keeps the first report focused on top-level record fields.
- `format=text` is human-readable in chat and the CLI.
- `sort=frequency` surfaces the most common fields first.
- `max_keys=0` means no surprise truncation.
- `samples=2`, `value_stats=true`, and `distinct=true` match a useful default profiling view.
- `invalid=report` gives forgiving exploration while still showing parse-quality problems.
- Page chips include an API-event log and a nested-array CSV case.

## Out-of-model / deliberately deferred

- Formal JSON Schema inference and required/optional schema emission.
- Browser file upload / streaming gigabyte files.
- Charts or histograms of field values.
- Cross-file diffing or schema drift over time.

## Verification focus

The final checks should prove:

1. Text output reports the exact record count and key coverage for a small NDJSON sample.
2. CSV/Markdown/JSON enum choices are exercised through CLI or page tests.
3. Nested depth produces dotted and `[]` array paths.
4. Invalid-line modes produce distinct behavior.
5. Page deep links prefill non-default enum, numeric, and checkbox controls and assert real output.

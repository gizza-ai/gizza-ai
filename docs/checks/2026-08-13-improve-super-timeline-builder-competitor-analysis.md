# super-timeline-builder — competitor analysis (2026-08-13)

Scan run before finalizing implementation. Notes are paraphrased observations only; no competitor copy, branding, or assets are reused.

## Competitor scan

1. Digital-forensics timeline tools commonly merge normalized CSV exports from multiple parsers, expand multi-timestamp file records into one event per timestamp, and sort everything chronologically. Their output is usually CSV-first so analysts can open it in spreadsheets and timeline viewers.
2. Legacy log2timeline/plaso-style workflows emphasize l2tcsv compatibility, source labels, MACB-style timestamp labels, host/user/path/message columns, UTC normalization, deduplication, and inclusive time-range filtering.
3. Incident-response notebooks and SIEM import helpers often accept multiple artifact tables, support descending/newest-first order, cap large exports, and keep the original source name so events can be traced back to the artifact that produced them.

## Table stakes and decisions

| Capability | Fit | Decision |
| --- | --- | --- |
| Multiple artifact CSVs in one paste | in-model | Section headers such as `--- mft ---`, `=== evtx ===`, GNU tail headers, or `# name` identify sources. |
| Per-section delimiter handling | in-model | `delimiter=auto` detects comma, tab, semicolon, or pipe per section; explicit enum override is available. |
| Timestamp-column detection | in-model | Header-name heuristics plus value parsing detect date/time columns; date+time split columns are recombined. |
| MFT-style expansion | in-model | `expand=true` emits one event per timestamp column and labels rows with the timestamp column name. |
| Chronological sorting | in-model | `order=asc|desc`. |
| UTC normalization | in-model | Absolute timestamps keep their offsets; `tz_offset` handles timezone-less inputs. |
| Inclusive range filtering | in-model | `from` and `to` date/time params. |
| Deduplication | in-model | `dedupe=true` removes identical `(time, source, type, message)` rows. |
| Null epoch suppression | in-model | `drop_epoch_zero` checkbox. |
| l2tcsv and TLN exports | in-model | `format=csv|l2tcsv|tln`. |
| Parser plugins for raw MFT/EVTX/browser databases | out-of-model | This tool merges already-parsed CSVs only; raw artifact parsers are separate tools. |
| Timezone databases, leap-second handling, analyst annotations | out-of-model | Keep deterministic UTC normalization with a numeric offset and text-first outputs. |

## Implementation stance

The shipped tool is a deterministic pure-Rust merger for parsed artifact tables. It does not parse raw binary forensic artifacts and it does not call a backend. The value is quick local normalization, expansion, sorting, filtering, and export to common timeline CSV shapes.

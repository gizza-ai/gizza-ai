# log-analyzer — competitor analysis (2026-07-13)

Scope: `log-analyzer` produces an **aggregate summary** of a log dump — counts by
level, top (grouped) errors, time span, and a bucketed volume timeline. It is the
summary counterpart to the sibling `log-parser` (row-by-row table/JSON/CSV).

## Competitors scanned (top 5 real, browser-based, paste-a-log tools)

1. **JSON Formatters Pro — Log Analyzer** (jsonformatterspro.com/log-analyzer) — paste/upload/sample,
   level filter, regex search, export.
2. **ToolsRail — Log File Analyzer** (toolsrail.com) — paste/upload, error highlight, "generate stats"
   and "visualize log trends".
3. **SwapCode — Log Viewer** (swapcode.ai/log-viewer) — auto format detect (timestamp/level/source/message),
   severity color-coding, date-range + severity + keyword filter.
4. **LogViewer.io** — fully in-browser (privacy), auto level highlight, spot critical issues.
5. **OnlyTools — Log File Analyzer** (onlytools.cc) — free, no registration, local processing.

## Table-stakes → where each lands

| Capability | Competitors | Ours | Decision |
| --- | --- | --- | --- |
| Auto-detect log format | 3, most | ✅ auto + 5 explicit formats (json/logfmt/syslog/common/combined) | in-model, shipped |
| Severity/level breakdown & counts | all | ✅ trace/debug/info/warn/error with share % | in-model, shipped |
| Error highlighting / "top errors" | 1,2 | ✅ top errors grouped by masking digits + hex ids, ranked by count | in-model, shipped (a differentiator — few group near-dup errors) |
| Stats / trends over time | 2 | ✅ volume timeline bucketed minute/hour/day (auto) with ASCII bars | in-model, shipped |
| Time span (first→last) | 3 | ✅ caption reports span | in-model, shipped |
| Machine-readable export | 1 | ✅ `output=json` structured object; page has a Download link | in-model, shipped |
| Sample / preset data | 1 | ✅ four `[[example]]` preset chips (JSON, combined access log, syslog, logfmt) | in-model, added on page |
| Runs locally / privacy | 4,5 | ✅ pure-Rust wasm, nothing uploaded | in-model, shipped |

## Out-of-model (listed, NOT built — no copy taken from any competitor)

- **Interactive syntax-highlighted, scrollable viewer** with per-row expansion / color coding
  (SwapCode, LogViewer) — that's an interactive UI, not a pure-compute transform; the row-level view
  is the sibling `log-parser`'s job (table/JSON/CSV). This tool is the aggregate summary.
- **Date-range picker / live tailing / very-large-file streaming** — needs stateful UI / a backend;
  paste into the page textarea covers the compute case.
- **Regex/keyword line filtering** — belongs to `log-parser` (which already ships `filter`+`regex`);
  duplicating it here would blur the two tools.

## Copy/UX gaps closed on this pass

- Added four preset example chips (one per major format) so users can try it in one click, matching
  competitors' "load sample" affordance.
- Friendly `<select>` labels for format/output/bucket (canonical values preserved for deep-links/CLI).
- SEO meta + hero + FAQ written fresh (no competitor copy/branding reused).

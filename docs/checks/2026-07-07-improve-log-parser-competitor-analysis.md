# log-parser — competitor analysis (2026-07-07)

Scan done BEFORE implementing, per the create-next-tool recipe. All findings are
paraphrased from public tool pages — no competitor copy, branding, or trademarks
reproduced. Out-of-model items are listed, not built.

## Tools scanned (browser-local log parsers, closest to gizza's model)

1. Devutils.lol — Log Parser (`devutils.lol/tools/log-parser`)
2. ToolsRail — Log File Analyzer (`toolsrail.com/file-tools/log-file-analyzer.php`)
3. Swapcode — Log Parser (`swapcode.ai/log-parser`)
   (also cross-checked: JSONFormattersPro Log Analyzer, pamburus/hl CLI viewer.)

## Table-stakes matrix

| Capability | Devutils | ToolsRail | Swapcode | In model? | Decision |
| --- | --- | --- | --- | --- | --- |
| Auto-detect format | yes (8 formats) | yes | yes (Apache/JSON/syslog) | in | **built** — `format=auto` majority-vote detector |
| JSON / NDJSON | yes | yes | yes | in | **built** |
| logfmt (key=value) | yes | — | — | in | **built** |
| Syslog (RFC 3164 + 5424) | yes | yes | yes | in | **built** |
| Access log — Common (CLF) | yes | yes | yes | in | **built** |
| Access log — Combined | yes | yes | yes | in | **built** |
| Explicit format override | yes (dropdown) | yes | yes | in | **built** — `format` enum `<select>` |
| Text / keyword search | yes | yes | yes | in | **built** — `filter` |
| Regex search | yes | yes | — | in | **built** — `regex=true` treats filter as a regex |
| Level filter (ERROR/WARN/INFO/DEBUG) | pills | checkboxes | dropdown | in | **built** — `level` enum (unified severity, min-threshold) |
| Combined/layered filters | yes | yes | yes | in | **built** — level + filter + limit compose |
| Stats (total / error / warn counts) | yes | yes | yes | in | **built** — table caption line |
| Export JSON | yes | — | yes | in | **built** — `output=json` |
| Export CSV | yes | (download) | yes | in | **built** — `output=csv` |
| Table display of structured fields | yes | yes | yes | in | **built** — `output=table` (Markdown, default) |
| Preset / "Load Sample" buttons | — | quick-actions | Load Sample | in | **built** — `[[example]]` chips (combined / JSON / logfmt / errors-only) |
| Row limit / cap | virtual scroll | virtual scroll | — | in | **built** — `limit` (default 200, max 5000) — a static text table can't virtual-scroll |

## Considered, not built (out-of-model or rejected)

- **Virtual scroll / 1GB+ files** — those tools stream into an interactive DOM grid; gizza
  renders one text/Markdown result. Mitigated with a `limit` cap and the honest page note.
- **Live tail / auto-scroll / dark-mode / colorize toggles** — interactive-viewer chrome, not
  a pure input→output transform. Out of model for a recompute-on-input tool.
- **Pinned bookmarks, collapsible multiline stack-trace grouping** — stateful viewer UI; a
  static table has no pin/expand affordance. Considered, rejected.
- **Time-range filter (Today / Last hour / custom)** — timestamps differ per format and many
  lines carry none; a robust cross-format time filter is fragile. Considered, rejected in favour
  of the unified severity + text/regex filters. Users can still filter timestamps with `filter`.
- **CSV-with-headers input, Docker/PM2/Python/Java framework log presets** — Docker JSON logs
  are already covered by the JSON path; CSV input belongs to the existing `csv-*` tools. Not added
  to avoid overlap.

## Notes

- ToolsRail states a soft ~50 MB browser limit; the others rely on virtual scroll. gizza states
  its own `limit` cap on the page rather than promising unbounded files.
- Unified severity is our differentiator over exact-level pills: `level=warn` keeps warnings **and**
  errors, and severity is derived consistently from JSON/logfmt level keys, syslog PRI, and HTTP
  status (5xx→error, 4xx→warn) so one filter works across all five formats.

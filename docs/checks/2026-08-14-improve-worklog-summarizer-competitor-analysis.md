# Competitor analysis — worklog-summarizer (2026-08-14)

Tool: `worklog-summarizer` — parses a timestamped activity ("doing") log where each entry runs until
the next timestamp, and totals the time per project, per tag and per day.

Scan performed 2026-08-14 (one web sweep over project time-tracking/report tooling, plus the
well-known CLI worklog reporters). No competitor copy, wording or branding is reproduced here or in
the shipped tool — only capability shapes are recorded.

## Competitors skimmed

| Competitor / tool shape | Table-stakes capabilities observed | UX/control patterns | Fit decision |
| --- | --- | --- | --- |
| CLI worklog reporters (timewarrior/watson/utt/timetrace-style `report` subcommands) | Totals per project and per tag; totals per day with a daily breakdown; date-range selection (from/to); duration shown as `h:mm` or decimal hours; entry-level listing with start/stop; CSV/JSON export for downstream tooling. | Report is a separate read-only command over an append-only log; range flags; a tag/project filter argument; stable column layout. | In model. Implemented `group_by` (all/project/tag/day/entry), `from`/`to`, `filter`, `units`, and `csv`/`json`/`table` outputs. |
| "Doing"-style / append-only activity logs (a line per activity, duration implied by the next line; org-mode clocktable) | Duration derived from consecutive timestamps rather than explicit start–stop pairs; `@`/`+`/`#` inline tags; explicit stop markers ("done", "end") that close the day without adding time; open (still running) last entry handled visibly. | Freeform paste; no schema up front; tolerant timestamp parsing (dates optional, times inherit the last date). | In model. This is the core parsing model: implied durations, midnight rollover for dateless lines, stop-marker vocabulary, and an explicit "open entry" warning plus an `end_time` to close it. |
| Project time-tracking dashboards (Timely/My Hours/Apploye-style) | Time per project/task/client for a chosen period; percentage-of-total share; sortable rankings; daily/weekly/monthly rollups; visual proportion bars; export. | Dashboard with charts, presets for common ranges, sort controls, export buttons. | Partly in model. Percentages, proportion bars (ASCII), sorting (`sort`), day rollups and CSV export are built. Charts, dashboards, saved reports, calendar sync, automatic activity capture and accounts are out of model. |
| Jira/issue-tracker worklog add-ons (WorklogPRO-style) | Rounding time to billing increments; per-person and per-issue rollups; approval workflows; invoice/rate calculations. | Grid editors, filters, scheduled report delivery. | Partly in model. Rounding to a billing increment (`round`, including the 6-minute tenth-of-an-hour step) is built. Per-person rollups, approvals, scheduled delivery and issue-tracker sync are out of model. Money/rate math intentionally stays in the sibling `timesheet-calculator` tool (explicit start–stop ranges plus rates), which this tool does not duplicate. |
| AI worklog summarizers (LLM narrative rollups) | Turn raw entries into a prose standup/status narrative; group semantically similar entries. | Chat prompt, model call per report. | Out of model. This block is pure Rust/WASM, local and deterministic — no model call. The deterministic totals it produces are exactly what an assistant can narrate afterwards. |

## Built requirements (table stakes landed in the descriptor)

- `log` paste area, 5,000,000-byte cap, tolerant timestamp parsing (`YYYY-MM-DD HH:MM[:SS]`, ISO `T`,
  `[bracketed]`, `HH:MM`-only, 12-hour `9:00am`, optional `-`/`–`/`|`/`:` separator).
- Duration implied by the next timestamp; stop markers (`done`, `end`, `stop`, `off`, `out`, `eod`,
  `---`) close an entry without adding time; dateless lines inherit the previous date and roll past
  midnight when the clock goes backwards.
- `group_by`: `all` (projects + tags + days), `project`, `tag`, `day`, `entry`.
- `output`: `summary` (readable, with proportion bars and percentages), `table`, `csv`, `json`.
- `units`: `hm` (`1h 30m`), `decimal` (`1.50`), `minutes`.
- `round`: `0`/`1`/`5`/`6`/`10`/`15`/`30`/`60`-minute increments applied per entry.
- `from` / `to` inclusive date range; `filter` comma list over project/tag with trailing-`*` prefix
  matching; `default_project` label for untagged entries; `sort` by `duration`, `name` or `time`.
- `end_time` closes a still-running final entry; otherwise the open entry is reported as open with
  zero time rather than silently guessed.

## Out-of-model / deliberately not built

- Live timers, start/stop daemons, automatic activity capture, editing entries in place.
- Accounts, teams, per-person rollups, approvals, scheduled or emailed reports.
- Integrations and sync (Jira, Toggl, calendars), invoicing, PDF/XLSX export.
- Charts/dashboards beyond deterministic ASCII proportion bars.
- Hourly rates and billable amounts — covered by the existing `timesheet-calculator` tool, which
  parses explicit `START-END` ranges; duplicating it here was rejected.
- LLM narrative summaries.

## Worked example used for checks

Input:

```text
2024-01-15 09:00 @acme +dev writing the parser
2024-01-15 10:30 @acme +review code review
2024-01-15 12:00 lunch
2024-01-15 13:00 @beta +dev bugfix
2024-01-15 17:00 done
```

Expected: `@acme` 3h 0m (37.5%), `@beta` 4h 0m (50.0%), `(untagged)` 1h 0m (12.5%, the lunch entry)
out of an 8h tracked day; tags `+dev` 5h 30m and `+review` 1h 30m; day `2024-01-15` 8h 0m; no open
entry (the `done` marker closes the day).

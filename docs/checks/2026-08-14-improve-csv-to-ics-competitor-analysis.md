# csv-to-ics — competitor analysis (2026-08-14)

Scan run before implementing the tool (build-time competitor pass per
`create-next-tool` step 4). One WebSearch for "CSV to ICS converter" →
skimmed the top reachable real competitors. **Everything below is paraphrased
observation of publicly documented behavior — no competitor copy, branding, or
trademark is reproduced or reused.**

## Competitors skimmed

| # | Tool | Reachable | Notes |
|---|------|-----------|-------|
| 1 | icalconverter.com/csv-to-ics | yes | Outlook-style column names, multi-format date parsing, UTC anchoring |
| 2 | jsoncsvtools.com/csv-to-ics | yes | Explicit column-mapping dropdowns, optional VTIMEZONE, global all-day toggle |
| 3 | csvtoics.app | yes | Widest alias vocabulary + widest date/time format list, preview, IANA picker |
| — | csv-to-ics.com | marketing page only | Field list deferred to a doc page that 404s; replaced by #3 per the "unreachable → replace, don't run with fewer" rule |

## Table stakes observed (and where each landed)

| Capability | Seen at | Our decision |
|---|---|---|
| Header auto-detection with alias vocabulary (Subject/Title/Summary/Event/Name; Start Date/Date/Start/Begin; Start Time/Time; End Date; End Time; All Day; Location/Place/Venue/Where; Description/Notes/Details) | 1,2,3 | **in-model — built.** Case/space/underscore-insensitive alias table in `core`. |
| Explicit column override when auto-detection guesses wrong (their dropdowns) | 2,3 | **in-model — built** as the `columns` param (`title=Task,start=When`), the declarative equivalent of a mapping dropdown. |
| ISO `YYYY-MM-DD` dates | 1,2,3 | **in-model — built.** |
| US `MM/DD/YYYY` and European `DD/MM/YYYY` | 1,2,3 | **in-model — built** via an explicit `date_order` enum (`auto`/`mdy`/`dmy`). `auto` resolves day-first when a component exceeds 12, else month-first — the same rule #3 documents. Made explicit because silent sampling-based auto-detect is the #1 source of wrong-month calendars. |
| Dotted European `DD.MM.YYYY` | 3 | **in-model — built.** |
| Written dates (`July 24, 2026`, `24 Jul 2026`) | 3 | **in-model — built.** |
| Two-digit years (`7/4/26` → 2026) | 1,3 | **in-model — built** (00–99 → 2000–2099, as #1 documents). |
| 24-hour, 12-hour AM/PM, `9am`, `9.30pm`, `HH:MM:SS`, `noon`, `midnight` | 3 | **in-model — built.** |
| All-day when the time column is blank | 1,2,3 | **in-model — built.** |
| Per-row all-day flag column (True/Yes/Y/1/X) | 1,2,3 | **in-model — built.** |
| Global "mark everything all-day" toggle | 2 | **in-model — built** as the `all_day` boolean. |
| `DTSTART;VALUE=DATE` + exclusive `DTEND` (+1 day) for all-day/multi-day | 1,3 | **in-model — built**, incl. the multi-day inclusive→exclusive adjustment. |
| IANA timezone selection | 2,3 | **in-model — built** (`timezone`, chrono-tz; already proven wasm-safe by `timezone-convert`). |
| UTC anchoring (`Z` times) | 1,3 | **in-model — built** — a named zone converts wall time → UTC and emits `Z`, which needs no `VTIMEZONE` and is unambiguous in every importer. |
| Floating local time (no zone) | 2 (default) | **in-model — built** — the default, so a calendar imported anywhere shows the pasted wall-clock time. |
| Default end when no end time is given | none documented a default | **in-model — built** as `default_duration` (minutes, default 60). Competitors leave this implicit; making it a parameter is a small genuine improvement. |
| RFC 5545 CRLF + 75-octet line folding | 2 | **in-model — built.** |
| Value escaping (`\, ; \n`) | implied | **in-model — built.** |
| Stable unique `UID` per event (re-import dedupe) | 2 flags it as a caveat | **in-model — built** — deterministic FNV-1a UID over the row's own content, so re-importing the same CSV updates rather than duplicates. |
| Multi-row-per-occurrence recurrence workaround | 1,2,3 (all: "no RRULE") | **in-model — built and better:** an optional `repeat` column emits a real `RRULE` (daily/weekly/monthly/yearly + `count`/`until` forms). No competitor scanned does this. |
| Reminders / `VALARM` | mentioned by the marketing page only | **in-model — built** as an optional `reminder` column (minutes before → `VALARM` with `DISPLAY`). |
| Column mapping preview of first N events | 3 | **considered, rejected** — the page's live output IS the preview (it re-runs on every keystroke and shows the full `.ics`), so a separate five-row preview pane would duplicate it. |
| Drag-and-drop file upload | 1,3 | **out-of-model for this tool** — pure tools take pasted text; a file-drop control is a shared generator capability, not a per-tool one. Pasting from a spreadsheet is the equivalent path. |
| Attendees / `ATTENDEE` + `ORGANIZER` | mentioned by the marketing page | **in-model — built** (`attendees` column, comma-separated emails; `organizer` param). |
| `URL`, `STATUS`, `CATEGORIES` columns | none of the three | **in-model — built** — cheap, standards-defined, and `ics-to-csv` in this repo already round-trips these columns, so the pair is now symmetric. |
| Generated `VTIMEZONE` blocks with DST rules | 2,3 | **considered, rejected** — emitting UTC `Z` times from a named zone is strictly RFC-compliant and importer-safe; hand-generating `VTIMEZONE` `RRULE` transition blocks adds a large failure surface for zero user-visible benefit. Stated on the page. |
| Cloud sync / account / saved templates / folder-watch automation | 1 (paid tier) | **out-of-model** — gizza is browser-local, no account, no server. Listed, not built. |

## Row/size limit

Capped at 5000 events per call (same bound `csv-to-vcard` uses), stated in the
descriptor, on the page, and in the error message.

## Sources

- https://icalconverter.com/csv-to-ics
- https://jsoncsvtools.com/csv-to-ics/
- https://csvtoics.app/
- https://www.csv-to-ics.com/ (marketing page; field documentation unreachable)

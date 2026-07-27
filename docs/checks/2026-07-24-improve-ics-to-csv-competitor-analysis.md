# ics-to-csv competitor analysis (2026-07-24)

Tool: `gizza-ai/ics-to-csv` — flatten iCalendar VEVENT records into spreadsheet-ready CSV.

## Competitor scan

| Source | Positioning | Table-stakes observed |
| --- | --- | --- |
| Online ICS/ICAL to CSV converters | Upload or paste an `.ics` file and download a spreadsheet. | One row per event, summary/start/end/location/description columns, CSV download/copy output, privacy concerns when upload-based. |
| Calendar export/import guides | Explain exporting calendars from Google/Apple/Outlook and converting them for spreadsheets. | Preserve start/end dates, support all-day events, keep UID/status/categories when present, handle folded lines and escaped commas/newlines. |
| Spreadsheet-oriented converter scripts | Developer/local workflows for batch conversion. | Delimiter options, optional header row, raw vs normalized date output, robust CSV quoting. |

## Fit-to-model decisions

| Capability / UX pattern | Decision | Rationale |
| --- | --- | --- |
| Pasted `.ics` text input | Built | Works in chat, CLI, and page without network or file-upload plumbing. |
| VEVENT rows | Built | Core requirement: one CSV row per event. |
| Summary/start/end/location/description columns | Built | Common spreadsheet columns users expect from calendar exports. |
| Optional status/categories/uid columns | Built | Included only when present so output stays compact but metadata is not lost. |
| Folded line unfolding | Built | Required for real RFC 5545 exports with long descriptions or summaries. |
| RFC 5545 TEXT unescaping | Built | Decodes escaped commas, semicolons, backslashes, and newlines before CSV escaping. |
| CSV delimiter choices | Built | Comma, semicolon, tab, and pipe cover spreadsheet locale and TSV workflows. |
| Header toggle | Built | Useful for imports that require headerless rows. |
| ISO/raw/unix date output | Built | Matches spreadsheet and scripting needs while keeping a raw escape hatch. |
| Timezone database / DST conversion | Out-of-model | Browser-local Rust block ships no tz database; `TZID`/floating values are normalized as wall-clock values or preserved with `raw`. |
| RRULE recurrence expansion | Out-of-model | Expanding recurrences requires full calendar recurrence semantics and timezone handling; this focused converter exports master events only. |
| Direct calendar URL fetching | Out-of-model | Network fetch introduces SSRF/network-surface concerns; users can paste exported `.ics` contents. |
| VTODO/VJOURNAL/VFREEBUSY export | Considered, rejected | The picked tool is event-to-spreadsheet; other component families have different columns and should be separate tools if needed. |

## Verification plan

- Unit tests cover normal timed events, all-day date-only events, multiple events, delimiter variants, header toggle, location/description toggles, status/categories/uid optional columns, folded lines, escaped multiline descriptions, nested VALARM skipping, quoted parameter parsing, raw/unix dates, pass-through unknown dates, and helpful errors.
- CLI checks should assert exact CSV output for default and non-default delimiter/header/date options.
- Page checks should assert exact output and query-param deep-link behavior.
